use anyhow::{Result, anyhow};
use clap::Parser;
use futures::SinkExt;
use log::{LevelFilter, Metadata, Record, SetLoggerError, debug};
use std::io::Write;
use tokio_stream::StreamExt;
use tokio_util::bytes::Buf;

const OPENSSH_PIPE_NAME: &str = r"\\.\pipe\openssh-ssh-agent";

/// SSH Agent Bridge - use Tokio forward stdin/stdout to Windows named pipe
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Named pipe name
    #[arg(short, long, default_value = OPENSSH_PIPE_NAME)]
    pipe: String,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Connection retry count
    #[arg(short, long, default_value = "30")]
    retries: u32,

    /// Retry delay (milliseconds)
    #[arg(long, default_value = "100")]
    retry_delay: u64,
}

const HEADER_SIZE: usize = std::mem::size_of::<u32>(); // SSH agent protocol uses a 4-byte length header

struct SshAgentMessage {
    length: u32,
    payload: Vec<u8>,
}
impl SshAgentMessage {
    fn new(length: u32, payload: Vec<u8>) -> Self {
        Self { length, payload }
    }
}

struct SshAgentCodec;
// impl tokio encoder decoder for SshAgentMessage
impl tokio_util::codec::Decoder for SshAgentCodec {
    type Item = SshAgentMessage;
    type Error = std::io::Error;

    fn decode(
        &mut self,
        src: &mut tokio_util::bytes::BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        debug!("Decoding message, buffer length: {}", src.len());
        if src.len() < HEADER_SIZE {
            return Ok(None);
        }
        let length = u32::from_be_bytes(src[..HEADER_SIZE].try_into().map_err(|_| {
            Self::Error::new(std::io::ErrorKind::InvalidData, "Failed to read length")
        })?);
        if src.len() < HEADER_SIZE + length as usize {
            return Ok(None);
        }
        src.advance(HEADER_SIZE);
        debug!("Decoding message, Message length: {}", length);
        let payload = src.split_to(length as usize).to_vec();
        Ok(Some(SshAgentMessage::new(length, payload)))
    }
}

impl tokio_util::codec::Encoder<SshAgentMessage> for SshAgentCodec {
    type Error = std::io::Error;
    fn encode(
        &mut self,
        item: SshAgentMessage,
        dst: &mut tokio_util::bytes::BytesMut,
    ) -> Result<(), Self::Error> {
        dst.extend_from_slice(&item.length.to_be_bytes());
        dst.extend_from_slice(&item.payload);
        debug!("Encoded message of length: {}", item.length);
        Ok(())
    }
}

// 辅助函数：转发流数据
async fn forward_stream<R, W>(
    reader: &mut tokio_util::codec::FramedRead<R, SshAgentCodec>,
    writer: &mut tokio_util::codec::FramedWrite<W, SshAgentCodec>,
    direction: &str,
) -> Result<bool>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    if let Some(msg) = reader.next().await {
        let msg = msg?;
        debug!("Forwarding {} message of length: {}", direction, msg.length);
        writer.send(msg).await?;
        writer.flush().await?;
        debug!("Flushed {}", direction);
        Ok(true)
    } else {
        debug!("{} stream closed", direction);
        Ok(false)
    }
}

/// 使用正确的 SSH 协议消息帧处理
async fn handle_ssh_protocol_framing() -> Result<()> {
    let cli = Cli::parse();

    debug!(
        "Starting SSH agent bridge with proper framing to: {}",
        cli.pipe
    );

    // 连接命名管道
    let pipe = tokio::task::spawn_blocking(move || {
        connect_to_named_pipe(&cli.pipe, cli.retries, cli.retry_delay)
    })
    .await??;

    debug!("Connected to named pipe successfully");

    // 转换为异步文件
    let pipe_file = tokio::fs::File::from_std(pipe);
    let (pipe_read, pipe_write) = tokio::io::split(pipe_file);
    let mut stdin_reader = tokio_util::codec::FramedRead::new(tokio::io::stdin(), SshAgentCodec);
    let mut stdout_writer = tokio_util::codec::FramedWrite::new(tokio::io::stdout(), SshAgentCodec);
    let mut pipe_reader = tokio_util::codec::FramedRead::new(pipe_read, SshAgentCodec);
    let mut pipe_writer = tokio_util::codec::FramedWrite::new(pipe_write, SshAgentCodec);

    loop {
        if !forward_stream(&mut stdin_reader, &mut pipe_writer, "[stdin -> pipe]").await? {
            break;
        }
        if !forward_stream(&mut pipe_reader, &mut stdout_writer, "[pipe -> stdout]").await? {
            break;
        }
    }

    debug!("SSH agent bridge terminated");
    Ok(())
}

/// 连接到 Windows 命名管道
fn connect_to_named_pipe(
    pipe_name: &str,
    max_retries: u32,
    retry_delay_ms: u64,
) -> Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    use std::time::Duration;
    const FILE_FLAG_OVERLAPPED: u32 = 0x40000000; // from winapi::um::winbase::FILE_FLAG_OVERLAPPED
    // 尝试多次连接
    for attempt in 0..=max_retries {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            // 使用 OVERLAPPED I/O 以便与异步代码兼容
            .custom_flags(FILE_FLAG_OVERLAPPED)
            .open(pipe_name)
        {
            Ok(pipe) => {
                debug!(
                    "Successfully connected to named pipe on attempt {}",
                    attempt + 1
                );
                return Ok(pipe);
            }
            Err(e) => {
                if attempt == max_retries {
                    return Err(anyhow!(
                        "Failed to connect to named pipe '{}' after {} attempts: {}",
                        pipe_name,
                        max_retries + 1,
                        e
                    ));
                }

                debug!(
                    "Connection attempt {} failed: {}, retrying in {}ms",
                    attempt + 1,
                    e,
                    retry_delay_ms
                );

                std::thread::sleep(Duration::from_millis(retry_delay_ms));
            }
        }
    }

    Err(anyhow!("Unexpected error connecting to named pipe"))
}

struct SimpleLogger;
impl log::Log for SimpleLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            eprintln!("{} - {}", record.level(), record.args());
        }
    }
    fn flush(&self) {
        std::io::stderr().flush().unwrap();
    }
}

pub fn log_init() -> Result<(), SetLoggerError> {
    log::set_logger(&SimpleLogger).map(|()| log::set_max_level(LevelFilter::Debug))
}
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.verbose {
        log_init()?;
    }
    debug!("Windows SSH Agent Bridge starting...");
    debug!("Target pipe: {}", cli.pipe);
    // check if the named pipe exists
    if let Err(_) = tokio::fs::metadata(&cli.pipe).await {
        debug!(
            "Warning: Named pipe '{}' does not exist or is not accessible",
            cli.pipe
        );
        debug!("Make sure SSH Agent is running on Windows");
        debug!("You can start it with: net start ssh-agent");
        return Err(anyhow!(
            "Named pipe '{}' does not exist or is not accessible",
            cli.pipe
        ));
    }

    handle_ssh_protocol_framing().await
}
