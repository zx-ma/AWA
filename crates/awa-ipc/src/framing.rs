use std::io::{Read, Write};

use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

const MAX_FRAME_LEN: usize = 1024 * 1024;

pub const DEFAULT_SOCKET_PATH: &str = "/run/awa/awa.sock";

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("frame too large: {0} bytes")]
    FrameTooLarge(usize),
}

pub fn write_json<W, T>(writer: &mut W, value: &T) -> Result<(), IpcError>
where
    W: Write,
    T: Serialize,
{
    let data = serde_json::to_vec(value)?;
    if data.len() > MAX_FRAME_LEN {
        return Err(IpcError::FrameTooLarge(data.len()));
    }

    let len = (data.len() as u32).to_be_bytes();
    writer.write_all(&len)?;
    writer.write_all(&data)?;
    writer.flush()?;
    Ok(())
}

pub fn read_json<R, T>(reader: &mut R) -> Result<T, IpcError>
where
    R: Read,
    T: DeserializeOwned,
{
    let mut len_buf = [0_u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_LEN {
        return Err(IpcError::FrameTooLarge(len));
    }

    let mut data = vec![0_u8; len];
    reader.read_exact(&mut data)?;
    Ok(serde_json::from_slice(&data)?)
}

#[cfg(feature = "async")]
pub async fn write_json_async<W, T>(writer: &mut W, value: &T) -> Result<(), IpcError>
where
    W: tokio::io::AsyncWrite + Unpin,
    T: Serialize,
{
    use tokio::io::AsyncWriteExt;

    let data = serde_json::to_vec(value)?;
    if data.len() > MAX_FRAME_LEN {
        return Err(IpcError::FrameTooLarge(data.len()));
    }

    writer.write_all(&(data.len() as u32).to_be_bytes()).await?;
    writer.write_all(&data).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(feature = "async")]
pub async fn read_json_async<R, T>(reader: &mut R) -> Result<T, IpcError>
where
    R: tokio::io::AsyncRead + Unpin,
    T: DeserializeOwned,
{
    use tokio::io::AsyncReadExt;

    let mut len_buf = [0_u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_LEN {
        return Err(IpcError::FrameTooLarge(len));
    }

    let mut data = vec![0_u8; len];
    reader.read_exact(&mut data).await?;
    Ok(serde_json::from_slice(&data)?)
}
