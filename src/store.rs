use core::cell::RefCell;
use serde::{Deserialize, Serialize};
use snafu::prelude::*;

use alloc::{rc::Rc, string::String};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel};
use framework::{error, info, mk_static, prelude::Framework, settings::{FILE_STORE_MAX_DIRS, FILE_STORE_MAX_FILES}, warn};

use crate::{bambu::TagInformation, csvdb::{CsvDb, CsvDbId}};


#[derive(Snafu, Debug)]
pub enum StoreError {
    #[snafu(display("Too many store operations pending"))]
    TooManyOps
}


#[derive(Debug)]
pub enum StoreOp {
    WriteTag(TagInformation),
}

type StoreRequestsChannel = Channel::<NoopRawMutex, StoreOp, 5>;
// type StoreRequestsReceiver<'a> = Receiver::<'a, NoopRawMutex, StoreOp, 5>;

// embedded_hal_bus::spi::ExclusiveDevice<esp_hal::spi::master::Spi<'_, esp_hal::Async>, esp_hal::gpio::Output<'_>, embedded_hal_bus::spi::NoDelay>
type TheSpi = embedded_hal_bus::spi::ExclusiveDevice<esp_hal::spi::master::Spi<'static, esp_hal::Async>, esp_hal::gpio::Output<'static>, embedded_hal_bus::spi::NoDelay>;

#[allow(private_interfaces)]
pub struct Store {
    pub requests_channel : &'static StoreRequestsChannel,
    // TODO: make spools_db mutext or something that doesn't need borrow 
    // Think if need to make the entire store under mutex (if there are several related dbs could case issues)
    pub spools_db: Option<CsvDb<SpoolRecord, TheSpi, 20, 5>>
}

impl Store {
    pub fn new(framework: Rc<RefCell<Framework>>) -> Rc<RefCell<Store>> {
        let requests_channel = mk_static!(StoreRequestsChannel, StoreRequestsChannel::new());
        let store = Rc::new(RefCell::new(Self {
            requests_channel,
            spools_db: None,
        }));
        framework.borrow().spawner.spawn(store_task(framework.clone(), store.clone())).ok();
        store
    }

    pub fn try_send_op(&self, op: StoreOp) -> Result<(), StoreError> {
        self.requests_channel.try_send(op).map_err(|_| StoreError::TooManyOps)
    }

    pub fn is_available(&self) -> bool {
        true
    }
}

#[embassy_executor::task] // up to two printers in parallel
pub async fn store_task(framework: Rc<RefCell<Framework>>, store: Rc<RefCell<Store>>) {
    {
        let file_store = framework.borrow().file_store();
        match CsvDb::<SpoolRecord, _, FILE_STORE_MAX_DIRS, FILE_STORE_MAX_FILES>::new(file_store.clone(), "/store/spools", 128, 200).await {
            Ok(db) => {
                store.borrow_mut().spools_db = Some(db);
                info!("Opened spools database");
            }
            Err(e) => {
                warn!("Failed to open spools database : {e}");
                return;
            }
        }
    }
    let receiver = store.borrow().requests_channel.receiver();
    loop {
        match receiver.receive().await {
            StoreOp::WriteTag(tag) => {
                if tag.tag_id.is_some() {
                    let spools_db = store.borrow_mut().spools_db.take();
                    if let Some(mut spools_db) = spools_db {
                        let spool_rec = SpoolRecord {
                            tag_id: tag.tag_id.unwrap(),
                            brand: tag.brand.unwrap_or_default(),
                            color_name: tag.color_name.unwrap_or_default(),
                        };
                        match spools_db.insert(spool_rec).await {
                            Ok(_) => {
                                info!("Stored tag into spools database");
                            }
                            Err(e) => {
                                error!("Error storing record to spools database {e}");
                            }
                        }
                        store.borrow_mut().spools_db = Some(spools_db); // return it to struct 
                    }
                }
            }
        }
    }
}


#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct SpoolRecord {
    pub tag_id: String,
    pub brand: String,
    pub color_name: String,
}

impl CsvDbId for SpoolRecord {

    fn id(&self) -> &String {
        &self.tag_id
    }
}
