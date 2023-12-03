use anyhow::{Context, Result};
use clap::Parser;
use log::{debug, Level};
use quinn;
use rustls::{Certificate, RootCertStore};
use rustls_native_certs;
use std::sync::{Arc, Mutex};
use tokio::task::JoinSet;
use webtransport_quinn;

mod media;
use media::*;

use moq_transport::model::broadcast::{self, Subscriber};

#[tokio::main]
async fn main() -> Result<()> {
	// Create a Config object with the desired server URI and port
	let bind_address = "[::]:0".parse().unwrap();
	let uri = "https://192.168.8.2:4443/demo".parse().unwrap();

	let (publisher, subscriber) = broadcast::new();
	let mut media = Media::new(subscriber).await?;

	// Load platform certificates
	let mut roots = rustls::RootCertStore::empty();
	for cert in rustls_native_certs::load_native_certs().context("could not load platform certs")? {
		roots.add(&Certificate(cert.0)).context("could not add certificate")?;
	}

	// Configure the TLS client
	let mut tls_config = rustls::ClientConfig::builder()
		.with_safe_defaults()
		.with_root_certificates(roots)
		.with_no_client_auth();

	tls_config.alpn_protocols = vec![webtransport_quinn::ALPN.to_vec()];

	let arc_tls_config = std::sync::Arc::new(tls_config);
	let quinn_client_config = quinn::ClientConfig::new(arc_tls_config);

	let mut endpoint = quinn::Endpoint::client(bind_address)?;
	endpoint.set_default_client_config(quinn_client_config);

	log::info!("connecting to {}", uri);

	// Create a WebTransport session
	let webtransport_session = webtransport_quinn::connect(&endpoint, &uri)
		.await
		.context("failed to create WebTransport session")?;

	let session = moq_transport::session::Client::subscriber(webtransport_session, publisher)
		.await
		.context("failed to create MoQ Transport session")?;

	// TODO run a task that returns a 404 for all unknown subscriptions.
	// get the media running

	tokio::select! {
		res = session.run() => res.context("session error")?,
		res = media.run() => res.context("media error")?,
	}

	Ok(())
}
