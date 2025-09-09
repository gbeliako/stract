// wet.rs
// Stract is an open source web search engine.
// Copyright (C) 2024 Stract ApS
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

use crate::{config::WarcSource, Error, Result};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read, Seek, Write};
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;

use tracing::{debug, trace};

// Add this to your error.rs file
// #[error("WET parse error: {0}")]
// WetParse(String),

pub struct WetFile {
    bytes: Vec<u8>,
}

fn rtrim(s: &mut String) {
    s.truncate(s.trim_end().len());
}

fn decode_string(raw: &[u8]) -> String {
    if let Ok(res) = String::from_utf8(raw.to_owned()) {
        res
    } else {
        String::from_utf8_lossy(raw).to_string()
    }
}

impl WetFile {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;

        Ok(Self::new(bytes))
    }

    pub fn records(&self) -> WetRecordIterator<&[u8]> {
        WetRecordIterator {
            reader: BufReader::new(MultiGzDecoder::new(&self.bytes[..])),
            num_reads: 0,
        }
    }

    pub(crate) fn download(source: &WarcSource, wet_path: &str) -> Result<Self> {
        let mut cursor = Cursor::new(Vec::new());
        Self::download_into_buf(source, wet_path, &mut cursor)?;
        cursor.rewind()?;

        let mut buf = Vec::new();
        cursor.read_to_end(&mut buf)?;

        Ok(Self::new(buf))
    }

    pub(crate) fn download_into_buf<W: Write + Seek>(
        source: &WarcSource,
        wet_path: &str,
        buf: &mut W,
    ) -> Result<()> {
        // You'll need to implement ExponentialBackoff or import it
        for dur in std::iter::repeat(Duration::from_millis(100)).take(5) {
            let res = match source {
                WarcSource::HTTP(config) => {
                    WetFile::download_from_http(wet_path, &config.base_url, buf)
                }
                WarcSource::Local(config) => {
                    WetFile::load_from_folder(wet_path, &config.folder, buf)
                }
                WarcSource::S3(config) => WetFile::download_from_s3(wet_path, config, buf),
            };

            if res.is_ok() {
                return Ok(());
            } else {
                trace!("Error {:?}", res);
            }

            debug!("wet download failed: {:?}", res.err().unwrap());
            debug!("retrying in {} ms", dur.as_millis());

            sleep(dur);
        }

        Err(Error::DownloadFailed.into())
    }

    fn load_from_folder<W: Write + Seek>(name: &str, folder: &str, buf: &mut W) -> Result<()> {
        let f = File::open(Path::new(folder).join(name))?;
        let mut reader = BufReader::new(f);

        buf.rewind()?;

        std::io::copy(&mut reader, buf)?;

        Ok(())
    }

    fn download_from_http<W: Write + Seek>(
        wet_path: &str,
        base_url: &str,
        buf: &mut W,
    ) -> Result<()> {
        let mut url = base_url.to_string();
        if !url.ends_with('/') {
            url += "/";
        }
        url += wet_path;

        let client = reqwest::blocking::ClientBuilder::new()
            .tcp_keepalive(None)
            .pool_idle_timeout(Duration::from_secs(30 * 60))
            .timeout(Duration::from_secs(30 * 60))
            .connect_timeout(Duration::from_secs(30 * 60))
            .build()?;
        let res = client.get(&url).send()?;

        if res.status().as_u16() != 200 {
            return Err(Error::DownloadFailed.into());
        }

        let bytes = res.bytes()?;

        buf.rewind()?;
        std::io::copy(&mut &bytes[..], buf)?;

        Ok(())
    }

    fn download_from_s3<W: Write + Seek>(
        wet_path: &str,
        config: &crate::config::S3Config,
        buf: &mut W,
    ) -> Result<()> {
        use s3::Bucket;
        use s3::creds::Credentials;
        use s3::region::Region;

        let bucket = Bucket::new(
            &config.bucket,
            Region::Custom {
                region: "".to_string(),
                endpoint: config.endpoint.clone(),
            },
            Credentials {
                access_key: Some(config.access_key.clone()),
                secret_key: Some(config.secret_key.clone()),
                security_token: None,
                session_token: None,
                expiration: None,
            },
        )?
        .with_path_style()
        .with_request_timeout(Duration::from_secs(30 * 60))?;

        let res = bucket.get_object_blocking(wet_path)?;

        buf.write_all(&res.bytes())?;

        Ok(())
    }
}

#[derive(Debug)]
struct RawWetRecord {
    header: BTreeMap<String, String>,
    content: Vec<u8>,
}

#[derive(Debug)]
pub struct WetRecord {
    pub url: String,
    pub body: String,
    pub fetch_time_ms: u64,
}

impl WetRecord {
    fn from_raw(record: RawWetRecord) -> Result<Self> {
        let content = decode_string(&record.content);
        
        // Parse the URL from the header
        let url = record.header
            .get("WARC-TARGET-URI")
            .ok_or_else(|| Error::WetParse("No target URI in WET record".to_string()))?
            .to_string();
        
        // Parse date if available (for fetch time approximation)
        let fetch_time_ms = record.header
            .get("WARC-DATE")
            .and_then(|d| {
                chrono::DateTime::parse_from_rfc3339(d)
                    .ok()
                    .map(|dt| dt.timestamp_millis() as u64)
            })
            .unwrap_or(0);
        
        Ok(Self {
            url,
            body: content,
            fetch_time_ms,
        })
    }
}

pub struct WetRecordIterator<R: Read> {
    reader: BufReader<MultiGzDecoder<R>>,
    num_reads: usize,
}

impl<R: Read> WetRecordIterator<R> {
    fn next_raw(&mut self) -> Option<Result<RawWetRecord>> {
        let mut version = String::new();

        if let Err(_io) = self.reader.read_line(&mut version) {
            return None;
        }

        if version.is_empty() {
            return None;
        }

        rtrim(&mut version);

        if !version.to_uppercase().starts_with("WARC/1.") {
            return Some(Err(
                Error::WetParse("Unknown WARC version".to_string()).into()
            ));
        }

        let mut header = BTreeMap::<String, String>::new();

        loop {
            let mut line_buf = String::new();
            if let Err(io) = self.reader.read_line(&mut line_buf) {
                return Some(Err(io.into()));
            }

            if &line_buf == "\r\n" || line_buf.is_empty() {
                // end of header
                break;
            }
            
            if let Some(semi) = line_buf.find(':') {
                let mut value = line_buf.split_off(semi + 1).to_string();

                if let Some(stripped) = value.strip_suffix("\r\n") {
                    value = stripped.to_string();
                } else if let Some(stripped) = value.strip_suffix('\n') {
                    value = stripped.to_string();
                }

                if let Some(stripped) = value.strip_prefix(' ') {
                    value = stripped.to_string();
                }

                line_buf.pop(); // remove colon
                let key = line_buf;

                header.insert(key.to_uppercase(), value);
            } else {
                return Some(Err(Error::WetParse(
                    "All header lines must contain a colon".to_string(),
                )
                .into()));
            }
        }

        let content_len = header.get("CONTENT-LENGTH");
        if content_len.is_none() {
            return Some(Err(Error::WetParse(
                "Record has no content-length".to_string(),
            )
            .into()));
        }

        let content_len = content_len.unwrap().parse::<usize>();
        if content_len.is_err() {
            return Some(Err(Error::WetParse(
                "Could not parse content length".to_string(),
            )
            .into()));
        }

        let content_len = content_len.unwrap();
        let mut content = vec![0; content_len];
        if let Err(io) = self.reader.read_exact(&mut content) {
            return Some(Err(io.into()));
        }

        let mut linefeed = [0u8; 4];
        if let Err(io) = self.reader.read_exact(&mut linefeed) {
            return Some(Err(io.into()));
        }

        if linefeed != [13, 10, 13, 10] {
            return Some(Err(
                Error::WetParse("Invalid record ending".to_string()).into()
            ));
        }

        let record = RawWetRecord { header, content };

        Some(Ok(record))
    }
}

impl<R: Read> Iterator for WetRecordIterator<R> {
    type Item = Result<WetRecord>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_reads == 0 {
            // Skip the warcinfo record
            if let Some(result) = self.next_raw() {
                if result.is_err() {
                    return Some(Err(result.err().unwrap()));
                }
            }
        }
        self.num_reads += 1;

        while let Some(item) = self.next_raw() {
            if item.is_err() {
                return Some(Err(item.err().unwrap()));
            }

            let item = item.unwrap();

            if let Some(warc_type) = item.header.get("WARC-TYPE") {
                if warc_type == "conversion" {
                    match WetRecord::from_raw(item) {
                        Ok(record) => return Some(Ok(record)),
                        Err(err) => return Some(Err(err)),
                    }
                }
            }
        }

        None
    }
}

// Add this to your error.rs file
// #[error("WET parse error: {0}")]
// WetParse(String),
