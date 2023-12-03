use anyhow::{self, Context, Ok};
use moq_transport::model::{broadcast, segment, track};
use mp4::{self, ReadBox};
use tokio::io::AsyncReadExt;
use csv::Writer;
use std::{time::{SystemTime, UNIX_EPOCH}, fs::OpenOptions};

pub struct Media {
	_subscriber: broadcast::Subscriber,
	track: moq_transport::model::track::Subscriber,
}

impl Media {
	pub async fn new(subscriber: broadcast::Subscriber) -> anyhow::Result<Self> {
		let new_track;

		match subscriber.get_track("1")? {
			track => {
				new_track = track;
			}
		}

		Ok(Media {
			_subscriber: subscriber,
			track: new_track,
		})
	}

	pub async fn run(&mut self) -> anyhow::Result<()> {
		// loop {
		// 	match self.track.next_segment().await? {
		// 		Some(mut segment) => loop {
		// 			let atom = read_atom(&mut segment).await?;

		// 			println!("{:?}", atom);
		// 		},
		// 		None => {}
		// 	}
		// }

		while let Some(mut segment) = self.track.next_segment().await? {
			tokio::spawn(async move {
				if let Err(err) = run_segment(&mut segment).await {
					// log::error!("failed to run segment: {:?}", err);
					print!("{:?}", err)
				}
			});
		}

		// loop {
		// 	let segment = self.track.next_segment().await?;
		// 	// unwrap the segment
		// 	let segment = segment.unwrap();

		// 	// match segment {
		// 	// 	Some(mut subscriber) => {
		// 	// 		let chunk = subscriber.read_chunk().await?;

		// 	// 		if let Some(chunk) = chunk {
		// 	// 			let mut reader = Cursor::new(chunk);

		// 	// 			let mut atom = read_atom(&mut reader).await?;

		// 	// 			print!("{:?}", atom);

		// 	// 			Ok(());
		// 	// 		}
		// 	// 	}
		// 	// 	None => {}
		// 	// }

		// 	// if let Some(subscriber) = self.track.next_segment().await? {
		// 	// 	// Access the data from the Subscriber instance using the get_data method
		// 	// 	match subscriber.get_data() {
		// 	// 		std::result::Result::Ok(data) => {
		// 	// 			// Use the data as needed
		// 	// 			println!("{:?}", data);
		// 	// 		}
		// 	// 		std::result::Result::Err(err) => {
		// 	// 			// Handle the error if there was a problem retrieving the data
		// 	// 			eprintln!("Error getting data: {:?}", err);
		// 	// 		}
		// 	// 	}
		// 	// } else {
		// 	// 	// Handle the case where next_segment returned None (no subscriber)
		// 	// }
		// }
		Ok(())
	}
}

// async fn run_segment(segment: &mut segment::Subscriber) -> anyhow::Result<()> {
// 	while let Some(chunk) = read_atom(segment).await? {
// 		log::info!("chunk: {:?}", chunk.len());
// 		println!("{:?}", chunk.len());
// 	}
// 	Ok(())
// }

async fn run_segment(segment: &mut segment::Subscriber) -> anyhow::Result<()> {
	let mut total_data_size: u64 = 0; // To track the total data size in the segment.
	let mut start_time = std::time::Instant::now();

	let mut file = OpenOptions::new().write(true).append(true).open("client3.csv")?;

	let mut csv_writer = Writer::from_writer(file);

	// Write CSV header
	// csv_writer.write_record(&["Timestamp", "Chunk Size (bytes)", "Bitrate (Mbps)"])?;

	while let Some(chunk) = read_atom(segment).await? {
		let now = SystemTime::now();
		let since_the_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

		// Convert to seconds (as f64)
		let seconds = since_the_epoch.as_secs() as f64;
		// Convert to nanoseconds (as f64) and divide by 1e9 to get fractional seconds
		let nanoseconds = since_the_epoch.subsec_nanos() as f64 / 1e9;
		// Combine seconds and nanoseconds to get the complete timestamp
		let timestamp = seconds + nanoseconds;

		let chunk_size = chunk.len() as u64;
		total_data_size += chunk_size;

		let elapsed_time = start_time.elapsed().as_secs_f64();
		if elapsed_time > 0.0 {
			let bitrate = (total_data_size * 8) as f64 / (elapsed_time * 1_000_000.0); // Calculate bitrate in Mbps.

			// Write data to the CSV file
			csv_writer.write_record(&[timestamp.to_string(), chunk_size.to_string(), format!("{:.2}", bitrate)])?;
			csv_writer.flush()?;

			log::info!(
				"Timestamp: {}, Chunk size: {} bytes, Bitrate: {:.2} Mbps",
				timestamp.to_string(),
				chunk_size,
				bitrate
			);
			println!(
				"Timestamp: {}, Chunk size: {} bytes, Bitrate: {:.2} Mbps",
				timestamp.to_string(),
				chunk_size,
				bitrate
			);
		} else {
			log::info!("Chunk size: {} bytes", chunk_size);
			println!("Chunk size: {} bytes", chunk_size);
		}
	}

	Ok(())
}


// Read a full MP4 atom into a vector.
async fn read_atom<R: AsyncReadExt + Unpin>(reader: &mut R) -> anyhow::Result<Option<Vec<u8>>> {
	// Read the 8 bytes for the size + type
	let mut buf = [0u8; 8];

	// Read up to 8 bytes.
	let n = reader.read(&mut buf).await?;

	// If we got 0 bytes, it's EOF.
	if n == 0 {
		return Ok(None);
	}

	// Read more if we didn't get all 8 bytes.
	reader.read_exact(&mut buf[n..]).await?;

	// Convert the first 4 bytes into the size.
	let size = u32::from_be_bytes(buf[0..4].try_into()?) as u64;

	let mut raw = buf.to_vec();

	let mut limit = match size {
		// Runs until the end of the file.
		0 => reader.take(u64::MAX),

		// The next 8 bytes are the extended size to be used instead.
		1 => {
			reader.read_exact(&mut buf).await?;
			let size_large = u64::from_be_bytes(buf);
			anyhow::ensure!(size_large >= 16, "impossible extended box size: {}", size_large);

			reader.take(size_large - 16)
		}

		2..=7 => {
			anyhow::bail!("impossible box size: {}", size)
		}

		size => reader.take(size - 8),
	};

	// Append to the vector and return it.
	let _read_bytes = limit.read_to_end(&mut raw).await?;

	Ok(Some(raw))
}
