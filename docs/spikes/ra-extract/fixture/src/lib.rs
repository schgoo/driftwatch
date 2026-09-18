use annotations::{watch_dep, watch_operation, watch_point, Watchable};
use std::collections::HashMap;
use std::{fmt, io};

pub type MyResult<T> = Result<T, MyError>;

#[derive(Debug, Clone)]
pub struct MyError;

#[derive(Debug, Clone, Watchable)]
pub struct User {
    #[watchable]
    pub id: u64,
    #[watchable]
    pub name: String,
}

#[derive(Debug, Clone, Watchable)]
pub struct Receipt {
    #[watchable]
    pub id: String,
    #[watchable]
    pub cents: u64,
}

#[derive(Debug, Clone, Watchable)]
pub enum ChargeError {
    Declined { code: u32 },
    Offline,
}

pub mod inventory {
    pub fn reserve(sku: &str) -> Result<u32, String> {
        Ok(sku.len() as u32)
    }
}

pub mod billing {
    use super::*;

    pub struct Service;

    impl Service {
        #[watch_operation(component = "billing")]
        pub fn method_charge(&self, cents: u64) -> Result<Receipt, ChargeError> {
            Ok(Receipt { id: cents.to_string(), cents })
        }
    }

    #[watch_operation(component = "billing")]
    fn private_io(name: String) -> io::Result<String> {
        Ok(name)
    }

    #[watch_operation(component = "billing")]
    pub fn anyhow_op(flag: bool) -> anyhow::Result<u32> {
        if flag { Ok(7) } else { Err(anyhow::anyhow!("nope")) }
    }

    #[watch_operation(component = "billing")]
    pub fn fmt_op() -> fmt::Result {
        Ok(())
    }

    #[watch_operation(component = "billing")]
    pub fn explicit_result(cents: u64) -> Result<Receipt, ChargeError> {
        let receipts = vec![Receipt { id: "r1".to_owned(), cents }];
        watch_point!("obs", &receipts);
        let _reserved = watch_dep!("reserve", component = "inventory", crate::inventory::reserve("sku-1"));
        Ok(receipts[0].clone())
    }

    #[watch_operation(component = "billing")]
    pub fn local_alias(user: User) -> MyResult<User> {
        Ok(user)
    }

    #[watch_operation(component = "billing")]
    pub fn option_user(id: u64) -> Option<User> {
        Some(User { id, name: "Ada".to_owned() })
    }

    #[watch_operation(component = "billing")]
    pub fn nested() -> (Vec<Option<User>>, HashMap<String, Vec<u8>>) {
        (Vec::new(), HashMap::new())
    }
}
