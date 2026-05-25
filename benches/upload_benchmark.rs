//! Starts `../sftp-s3-rs` with its memory backend unless `SFTP_BENCH_ADDR`
//! points at an external server. Useful knobs: `SFTP_BENCH_USER`,
//! `SFTP_BENCH_PASSWORD`, `SFTP_BENCH_DIR`, `SFTP_BENCH_CASES=1x1MiB,8x10MiB`,
//! `SFTP_BENCH_SERVER_DIR`, and `SFTP_BENCH_SERVER_BIN`.

use anyhow::{bail, Context, Result};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use log::debug;
use russh::{client, ChannelId};
use russh_sftp::client::{fs::File, SftpSession};
use std::{
    env, fmt, fs,
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{self, Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use tokio::{io::AsyncWriteExt, task};

const MIB: usize = 1024 * 1024;
const MANAGED_SERVER_USER: &str = "benchmark";
const MANAGED_SERVER_PASSWORD: &str = "benchmark";
const SERVER_READY_TIMEOUT: Duration = Duration::from_secs(10);
const MANAGED_SERVER_START_ATTEMPTS: usize = 5;
static RUN_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
struct BenchConfig {
    addr: String,
    username: String,
    password: String,
    remote_dir: String,
}

struct BenchServer {
    child: Child,
    log_path: PathBuf,
}

impl Drop for BenchServer {
    fn drop(&mut self) {
        match self.child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) => {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
            Err(error) => {
                eprintln!(
                    "failed to inspect sftp-s3 benchmark server process; log: {}: {error}",
                    self.log_path.display()
                );
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct UploadCase {
    file_count: usize,
    file_size: usize,
}

impl UploadCase {
    fn bytes(self) -> u64 {
        (self.file_count as u64) * (self.file_size as u64)
    }
}

impl fmt::Display for UploadCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}_files_x_{}_bytes", self.file_count, self.file_size)
    }
}

struct Client;

impl client::Handler for Client {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        debug!("check_server_key: {:?}", server_public_key);
        Ok(true)
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        debug!("data on channel {:?}: {}", channel, data.len());
        Ok(())
    }
}

fn configured_bench_target() -> Result<(BenchConfig, Option<BenchServer>)> {
    if env::var_os("SFTP_BENCH_ADDR").is_some() {
        return Ok((external_bench_config(), None));
    }

    let (config, server) = start_managed_sftp_server()?;
    Ok((config, Some(server)))
}

fn external_bench_config() -> BenchConfig {
    BenchConfig {
        addr: env::var("SFTP_BENCH_ADDR").expect("SFTP_BENCH_ADDR must be set"),
        username: env::var("SFTP_BENCH_USER").unwrap_or_else(|_| "root".to_string()),
        password: env::var("SFTP_BENCH_PASSWORD").unwrap_or_else(|_| "password".to_string()),
        remote_dir: env::var("SFTP_BENCH_DIR").unwrap_or_else(|_| ".".to_string()),
    }
}

fn start_managed_sftp_server() -> Result<(BenchConfig, BenchServer)> {
    let server_dir = sftp_s3_dir()?;
    let server_bin = sftp_s3_binary(&server_dir);

    if env::var_os("SFTP_BENCH_SERVER_BIN").is_none()
        && env::var_os("SFTP_BENCH_SKIP_SERVER_BUILD").is_none()
    {
        build_sftp_s3_server(&server_dir)?;
    }

    let log_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("sftp-bench-server.log");

    for attempt in 1..=MANAGED_SERVER_START_ATTEMPTS {
        let port = pick_available_port()?;
        let log = open_server_log(&log_path)?;
        let child = Command::new(&server_bin)
            .args([
                "--backend",
                "memory",
                "--port",
                &port.to_string(),
                "--user",
                &format!("{MANAGED_SERVER_USER}:{MANAGED_SERVER_PASSWORD}"),
            ])
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()
            .with_context(|| format!("failed to start {}", server_bin.display()))?;

        let mut server = BenchServer {
            child,
            log_path: log_path.clone(),
        };

        match wait_for_server("127.0.0.1", port, &mut server) {
            Ok(()) => {
                return Ok((
                    BenchConfig {
                        addr: format!("127.0.0.1:{port}"),
                        username: env::var("SFTP_BENCH_USER")
                            .unwrap_or_else(|_| MANAGED_SERVER_USER.to_string()),
                        password: env::var("SFTP_BENCH_PASSWORD")
                            .unwrap_or_else(|_| MANAGED_SERVER_PASSWORD.to_string()),
                        remote_dir: env::var("SFTP_BENCH_DIR").unwrap_or_else(|_| ".".to_string()),
                    },
                    server,
                ));
            }
            Err(err) if attempt < MANAGED_SERVER_START_ATTEMPTS => {
                drop(server);
                debug!("managed benchmark server startup attempt {attempt} failed: {err}");
            }
            Err(err) => return Err(err),
        }
    }

    unreachable!("managed server start attempts loop always returns")
}

fn sftp_s3_dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os("SFTP_BENCH_SERVER_DIR") {
        return Ok(PathBuf::from(path));
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let projects_dir = manifest_dir
        .parent()
        .context("failed to find parent directory for russh-sftp")?;
    Ok(projects_dir.join("sftp-s3-rs"))
}

fn sftp_s3_binary(server_dir: &Path) -> PathBuf {
    env::var_os("SFTP_BENCH_SERVER_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            server_dir
                .join("target")
                .join("release")
                .join(format!("sftp-s3{}", env::consts::EXE_SUFFIX))
        })
}

fn build_sftp_s3_server(server_dir: &Path) -> Result<()> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let manifest_path = server_dir.join("Cargo.toml");
    let status = Command::new(cargo)
        .args([
            "build",
            "--release",
            "--no-default-features",
            "--bin",
            "sftp-s3",
            "--manifest-path",
        ])
        .arg(&manifest_path)
        .status()
        .with_context(|| format!("failed to run cargo build for {}", server_dir.display()))?;

    if !status.success() {
        bail!(
            "failed to build sftp-s3 benchmark server from {}",
            manifest_path.display()
        );
    }

    Ok(())
}

fn pick_available_port() -> Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .context("failed to reserve an ephemeral benchmark server port")?;
    Ok(listener.local_addr()?.port())
}

fn open_server_log(path: &Path) -> Result<fs::File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))
}

fn wait_for_server(host: &str, port: u16, server: &mut BenchServer) -> Result<()> {
    let start = Instant::now();

    while start.elapsed() < SERVER_READY_TIMEOUT {
        if let Some(status) = server.child.try_wait()? {
            bail!(
                "sftp-s3 benchmark server exited during startup with {status}; log: {}",
                server.log_path.display()
            );
        }

        if TcpStream::connect((host, port)).is_ok() {
            return Ok(());
        }

        thread::sleep(Duration::from_millis(50));
    }

    bail!(
        "sftp-s3 benchmark server did not become ready within {:?}; log: {}",
        SERVER_READY_TIMEOUT,
        server.log_path.display()
    )
}

fn benchmark_cases() -> Vec<UploadCase> {
    match env::var("SFTP_BENCH_CASES") {
        Ok(raw) => parse_cases(&raw).expect("invalid SFTP_BENCH_CASES"),
        Err(_) => vec![
            UploadCase {
                file_count: 1,
                file_size: MIB,
            },
            UploadCase {
                file_count: 8,
                file_size: 10 * MIB,
            },
        ],
    }
}

fn parse_cases(raw: &str) -> Result<Vec<UploadCase>> {
    let cases = raw
        .split(',')
        .filter(|case| !case.trim().is_empty())
        .map(parse_case)
        .collect::<Result<Vec<_>>>()?;

    if cases.is_empty() {
        bail!("SFTP_BENCH_CASES must contain at least one case");
    }

    Ok(cases)
}

fn parse_case(raw: &str) -> Result<UploadCase> {
    let (file_count, file_size) = raw
        .trim()
        .split_once('x')
        .or_else(|| raw.trim().split_once('X'))
        .with_context(|| format!("case '{raw}' must use '<file_count>x<file_size>'"))?;

    let file_count = file_count
        .trim()
        .parse::<usize>()
        .with_context(|| format!("invalid file count in case '{raw}'"))?;
    let file_size =
        parse_size(file_size).with_context(|| format!("invalid file size in case '{raw}'"))?;

    if file_count == 0 || file_size == 0 {
        bail!("case '{raw}' must use non-zero values");
    }

    Ok(UploadCase {
        file_count,
        file_size,
    })
}

fn parse_size(raw: &str) -> Result<usize> {
    let raw = raw.trim();
    let lower = raw.to_ascii_lowercase();
    let (number, multiplier) = if let Some(number) = lower.strip_suffix("mib") {
        (number, MIB)
    } else if let Some(number) = lower.strip_suffix("kib") {
        (number, 1024)
    } else if let Some(number) = lower.strip_suffix('b') {
        (number, 1)
    } else {
        (lower.as_str(), 1)
    };

    number
        .trim()
        .parse::<usize>()?
        .checked_mul(multiplier)
        .with_context(|| format!("file size '{raw}' is too large"))
}

async fn connect_sftp(config: &BenchConfig) -> Result<SftpSession> {
    let ssh_config = russh::client::Config::default();
    let sh = Client {};
    let mut session = russh::client::connect(Arc::new(ssh_config), config.addr.as_str(), sh)
        .await
        .with_context(|| format!("failed to connect to {}", config.addr))?;

    let auth = session
        .authenticate_password(&config.username, &config.password)
        .await
        .with_context(|| format!("failed to authenticate as {}", config.username))?;

    if !auth.success() {
        bail!("password authentication failed for {}", config.username);
    }

    let channel = session
        .channel_open_session()
        .await
        .context("failed to open SSH session channel")?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .context("failed to request SFTP subsystem")?;

    SftpSession::new(channel.into_stream())
        .await
        .context("failed to initialize SFTP session")
}

fn benchmark_paths(
    config: &BenchConfig,
    case: UploadCase,
    run_id: u64,
    iteration: u64,
) -> Vec<String> {
    let base_name = format!("russh-sftp-bench-{}-{run_id}-{iteration}", process::id());

    (0..case.file_count)
        .map(|file_index| remote_path(&config.remote_dir, &format!("{base_name}-{file_index}.bin")))
        .collect()
}

fn remote_path(remote_dir: &str, file_name: &str) -> String {
    if remote_dir == "/" {
        return format!("/{file_name}");
    }

    match remote_dir.trim_end_matches('/') {
        "" | "." => file_name.to_string(),
        dir => format!("{dir}/{file_name}"),
    }
}

async fn create_files(sftp: &SftpSession, paths: &[String]) -> Result<Vec<File>> {
    let mut files = Vec::with_capacity(paths.len());

    for path in paths {
        files.push(
            sftp.create(path.clone())
                .await
                .with_context(|| format!("failed to create remote file {path}"))?,
        );
    }

    Ok(files)
}

async fn write_files(files: Vec<File>, data: Arc<Vec<u8>>) -> Result<()> {
    let mut handles = Vec::with_capacity(files.len());

    for mut file in files {
        let data = data.clone();
        handles.push(task::spawn(async move {
            file.write_all(data.as_slice()).await?;
            file.shutdown().await?;
            Result::<()>::Ok(())
        }));
    }

    for handle in handles {
        handle.await.context("upload task panicked")??;
    }

    Ok(())
}

async fn remove_files(sftp: &SftpSession, paths: &[String]) -> Result<()> {
    for path in paths {
        sftp.remove_file(path.clone())
            .await
            .with_context(|| format!("failed to remove remote file {path}"))?;
    }

    Ok(())
}

async fn run_upload_iteration(
    config: &BenchConfig,
    sftp: &SftpSession,
    case: UploadCase,
    data: Arc<Vec<u8>>,
    run_id: u64,
    iteration: u64,
) -> Result<Duration> {
    let paths = benchmark_paths(config, case, run_id, iteration);
    let files = match create_files(sftp, &paths).await {
        Ok(files) => files,
        Err(error) => {
            let _ = remove_files(sftp, &paths).await;
            return Err(error);
        }
    };

    let start = Instant::now();
    let write_result = write_files(files, data).await;
    let elapsed = start.elapsed();
    let cleanup_result = remove_files(sftp, &paths).await;

    write_result?;
    cleanup_result?;

    Ok(elapsed)
}

fn criterion_benchmark_upload(c: &mut Criterion) {
    let (config, _server) = configured_bench_target().expect("failed to prepare benchmark target");
    let cases = benchmark_cases();
    let mut group = c.benchmark_group("sftp_upload_write_all");
    group.sample_size(10);

    for case in cases {
        group.throughput(Throughput::Bytes(case.bytes()));
        group.bench_with_input(BenchmarkId::from_parameter(case), &case, |b, &case| {
            let runtime = tokio::runtime::Runtime::new().expect("failed to create Tokio runtime");
            let config = config.clone();

            b.to_async(runtime).iter_custom(move |iters| {
                let config = config.clone();

                async move {
                    let sftp = connect_sftp(&config).await.expect("failed to connect SFTP");
                    let data = Arc::new(vec![0; case.file_size]);
                    let run_id = RUN_ID.fetch_add(1, Ordering::Relaxed);
                    let mut elapsed = Duration::ZERO;

                    for iteration in 0..iters {
                        elapsed += run_upload_iteration(
                            &config,
                            &sftp,
                            case,
                            data.clone(),
                            run_id,
                            iteration,
                        )
                        .await
                        .expect("SFTP upload benchmark iteration failed");
                    }

                    sftp.close().await.expect("failed to close SFTP session");
                    elapsed
                }
            })
        });
    }

    group.finish();
}

criterion_group!(
    name = upload_benches;
    config = Criterion::default();
    targets = criterion_benchmark_upload
);
criterion_main!(upload_benches);
