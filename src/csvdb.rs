use core::num::ParseIntError;

use framework::prelude::{SDCardStore, SDCardStoreErrorSource};
use log::info;

use alloc::{
    format,
    rc::Rc,
    string::String,
    vec::Vec,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embedded_hal_async::spi::SpiDevice;
use hashbrown::HashMap;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

// use crate::sdcard_store::{SDCardStore, SDCardStoreErrorType};

use snafu::prelude::*;

#[derive(Snafu, Debug)]
pub enum CsvDbError
{
    #[snafu(display("Failed to open volume"))]
    Store {
        source: SDCardStoreErrorSource,
    },

    Metadata {
        source: ParseIntError,
    },

    Data {
        source: serde_csv_core::de::Error,
    }
}

pub trait CsvDbId {
    fn id(&self) -> &String;
}

#[derive(Debug)]
struct CsvRecordInfo<T>
where
    T: PartialEq + core::fmt::Debug,
{
    data: T,
    offset: u32,
}

#[derive(Serialize, Deserialize)]
struct DbMeta {
    version: usize,
    record_width: usize,
}

pub struct CsvDb<T, SPI: SpiDevice, const MAX_DIRS: usize, const MAX_FILES: usize>
where
    T: CsvDbId + Serialize + DeserializeOwned + PartialEq + core::fmt::Debug,
{
    sdcard: Rc<Mutex<CriticalSectionRawMutex, SDCardStore<SPI, MAX_DIRS, MAX_FILES>>>,
    db_file_name: String,
    _dbm_file_name: String,
    records: HashMap<String, CsvRecordInfo<T>>,
    buffer: Vec<u8>,
    record_width: usize,
}

impl<T, SPI: SpiDevice, const MAX_DIRS: usize, const MAX_FILES: usize>
    CsvDb<T, SPI, MAX_DIRS, MAX_FILES>
where
    T: CsvDbId + Serialize + DeserializeOwned + PartialEq + core::fmt::Debug,
{
    pub async fn new(
        sdcard: Rc<Mutex<CriticalSectionRawMutex, SDCardStore<SPI, MAX_DIRS, MAX_FILES>>>,
        db_name: &str,
        min_record_width: usize,
        min_capacity: usize,
    ) -> Result<Self, CsvDbError> {
        let dbm_file_name = format!("{db_name}.dbm");
        let db_file_name = format!("{db_name}.db");
        let sdcard_input = sdcard.clone();
        let mut record_width = min_record_width;
        let mut records = HashMap::<String, CsvRecordInfo<T>>::with_capacity(min_capacity);

        let mut sdcard = sdcard.lock().await;
        let dbm_str = sdcard.read_create_str(&dbm_file_name).await.context(StoreSnafu)?;
        if dbm_str.is_empty() {
            let dbm_str = format!("version: 1\nrecord_width:{min_record_width}");
            sdcard.append_text(&dbm_file_name, &dbm_str).await.context(StoreSnafu)?;
            sdcard.create_file(&db_file_name).await.context(StoreSnafu)?;
        } else {
            // Get relevant info from dbm file
            let mut lines = dbm_str.lines();
            let _line1 = lines.next();
            let line2 = lines.next();
            if let Some(line2) = line2 {
                if let Some((_left, right)) = line2.split_once(':') {
                    record_width = right.trim().parse().context(MetadataSnafu)?;
                }
            }
            // Now read db file
            let db_bytes = sdcard.read_create_bytes(&db_file_name).await.context(StoreSnafu)?;
            let mut reader = serde_csv_core::Reader::<100>::new(); // 100 is max field size
            let mut nread = 0;
            while nread < db_bytes.len() {
                let db_record = &db_bytes[nread..nread + record_width];
                if !Self::is_empty_record(db_record) {
                    let (record, _n) = reader.deserialize::<T>(db_record).context(DataSnafu)?;
                    let record_info = CsvRecordInfo {
                        data: record,
                        offset: nread as u32,
                    };
                    records.insert(record_info.data.id().clone(), record_info);
                }
                nread += record_width;
            }
            info!("Done reading: {records:?}");
        }

        Ok(Self {
            db_file_name,
            _dbm_file_name: dbm_file_name,
            records,
            sdcard: sdcard_input.clone(),
            record_width,
            buffer: Vec::with_capacity(record_width),
        })
    }

    pub async fn insert(&mut self, record: T) -> Result<(), CsvDbError> {
        self.calc_csv_row(&record);
        if let Some(v) = self.records.get_mut(record.id()) {
            if v.data != record {
                v.data = record;
                let mut sdcard = self.sdcard.lock().await;
                sdcard
                    .write_file_bytes(&self.db_file_name, v.offset, self.buffer.as_slice())
                    .await.context(StoreSnafu)?;
            } else {
                info!("Records are the same, skipping update");
            }
        } else {
            let mut sdcard = self.sdcard.lock().await;
            let offset = sdcard
                .append_bytes(&self.db_file_name, self.buffer.as_slice())
                .await.context(StoreSnafu)?;
            let csv_record_info = CsvRecordInfo {
                data: record,
                offset,
            };
            self.records
                .insert(csv_record_info.data.id().clone(), csv_record_info);
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete(&mut self, id: &str) -> Result<Option<T>, CsvDbError> {
        self.calc_empty_record();
        if let Some(record) = self.records.remove(id) {
            let mut sdcard = self.sdcard.lock().await;
            sdcard
                .write_file_bytes(&self.db_file_name, record.offset, self.buffer.as_slice())
                .await.context(StoreSnafu)?;
            return Ok(Some(record.data));
        } else {
            info!("Not found to delete {id}");
        }
        Ok(None)
    }

    fn calc_csv_row(&mut self, record: &T) {
        let mut writer = serde_csv_core::Writer::new();
        self.calc_empty_record();
        let _nwritten = writer.serialize(record, self.buffer.as_mut_slice());
    }

    fn calc_empty_record(&mut self) {
        self.buffer.resize(self.record_width, b'-');
        self.buffer[self.record_width-1] = b'\n';
    }

    fn is_empty_record(s: &[u8]) -> bool {
        s.len() > 1 && s[s.len() - 1] == b'\n' && s[..s.len() - 1].iter().all(|&c| c == b'-')
    }
}
