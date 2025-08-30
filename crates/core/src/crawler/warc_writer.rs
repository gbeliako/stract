// Stract is an open source web search engine.
// Copyright (C) 2023 Stract ApS
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//use std::time::Duration;

//nuevo
use std::path::PathBuf;

use crate::{
//    config::{self, S3Config},
//    config,
    warc,
};

use super::{CrawlDatum, DatumSink, Error, Result};
use anyhow::anyhow;

/// The WarcWriter is responsible for storing the crawl datums
/// as WARC files on S3.
pub struct WarcWriter {
    tx: tokio::sync::mpsc::Sender<WarcWriterMessage>,
}

impl DatumSink for WarcWriter {
    async fn write(&self, crawl_datum: CrawlDatum) -> Result<()> {
        self.tx
            .send(WarcWriterMessage::Crawl(crawl_datum))
            .await
            .map_err(|e| Error::from(anyhow!(e)))?;

        Ok(())
    }

    async fn finish(&self) -> Result<()> {
        self.tx
            .send(WarcWriterMessage::Finish)
            .await
            .map_err(|e| Error::from(anyhow!(e)))?;
        self.tx.closed().await;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum WarcWriterMessage {
    Crawl(CrawlDatum),
    Finish,
}

//async fn commit(writer: warc::DeduplicatedWarcWriter, s3: config::S3Config) {
async fn commit(writer: warc::DeduplicatedWarcWriter, output_dir: PathBuf) {
    let filename = format!(
        "{}_{}.warc.gz",
        chrono::Utc::now().to_rfc3339(),
        uuid::Uuid::new_v4()
    );
    let data = writer.finish().unwrap();

//    match s3::Bucket::new(
//        &s3.bucket,
//        s3::Region::Custom {
//            region: "".to_string(),
//            endpoint: s3.endpoint.clone(),
//        },
//        s3::creds::Credentials {
//            access_key: Some(s3.access_key.clone()),
//            secret_key: Some(s3.secret_key.clone()),
//            security_token: None,
//            session_token: None,
//            expiration: None,
//        },
//    ) {
//        Ok(bucket) => {
//            let bucket = bucket
//                .with_path_style()
//                .with_request_timeout(Duration::from_secs(30 * 60))
//                .unwrap();
//
//            if let Err(err) = bucket
//                .put_object_with_content_type(
//                    &format!("{}/{}", &s3.folder, filename),
//                    &data,
//                    "application/warc",
//                )
//                .await
//            {
//                tracing::error!("failed to upload to bucket: {:?}", err);
//            }
//        }
//        Err(err) => tracing::error!("failed to connect to bucket: {:?}", err),


//Nuevo
    // Create output directory if it doesn't exist
    if let Err(e) = tokio::fs::create_dir_all(&output_dir).await {
        tracing::error!("Failed to create directory {}: {:?}", output_dir.display(), e);
        return;
    }

    let file_path = output_dir.join(filename);

    match tokio::fs::write(&file_path, data).await {
        Ok(_) => tracing::info!("WARC file written to: {}", file_path.display()),
        Err(e) => tracing::error!("Failed to write WARC file: {:?}", e),

    }
}

//async fn writer_task(mut rx: tokio::sync::mpsc::Receiver<WarcWriterMessage>, s3: S3Config) {
async fn writer_task(mut rx: tokio::sync::mpsc::Receiver<WarcWriterMessage>, output_dir: PathBuf) {
    let mut writer = warc::DeduplicatedWarcWriter::new();
    let output_dir = output_dir.clone(); // Clone for use in the loop

    while let Some(message) = rx.recv().await {
        match message {
            WarcWriterMessage::Crawl(datum) => {
                let w = &mut writer;
                let (send, recv) = tokio::sync::oneshot::channel();

                rayon::scope(move |s| {
                    s.spawn(move |_| {
                        let warc_record = warc::WarcRecord {
                            request: warc::Request {
                                url: datum.url.to_string(),
                                date: Some(datum.date),
                            },
                            response: warc::Response {
                                body: datum.body,
                                payload_type: Some(datum.payload_type),
                            },
                            metadata: warc::Metadata {
                                fetch_time_ms: datum.fetch_time_ms,
                            },
                        };

                        w.write(&warc_record).unwrap();
                        send.send(()).unwrap();
                    });
                });

                recv.await.unwrap();

                if writer.num_bytes() > 1_000_000_000 {
//                    commit(writer, s3.clone()).await;
                    commit(writer, output_dir.clone()).await;
                    writer = warc::DeduplicatedWarcWriter::new();
                }
            }
            WarcWriterMessage::Finish => {
                if writer.num_writes() > 0 {
//                    commit(writer, s3.clone()).await;
                    commit(writer, output_dir).await;
                }
                break;
            }
        }
    }
}

impl WarcWriter {
//    pub fn new(s3: S3Config) -> Self {
    pub fn new(output_dir: PathBuf) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(1);

        tokio::spawn(writer_task(rx, output_dir));

        Self { tx }
    }
}
