use crate::coding::{decode_string, encode_string, AsyncRead, AsyncWrite, DecodeError, EncodeError};

/// Sent by the subscriber to accept an Announce.
#[derive(Clone, Debug)]
pub struct AnnounceOk {
	// Echo back the namespace that was announced.
	// TODO Propose using an ID to save bytes.
	pub namespace: String,
}

impl AnnounceOk {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let namespace = decode_string(r).await?;
		Ok(Self { namespace })
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		encode_string(&self.namespace, w).await
	}
}
