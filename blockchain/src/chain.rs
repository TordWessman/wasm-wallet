use std::error::Error;
use std::sync::{Weak, Mutex, Arc};
use std::marker::{Send, Sync};

pub type BalanceObserver = dyn BalanceUpdatedObserver + Send + Sync;
pub type ObserverList = Arc<Mutex<Vec<Weak<Mutex<BalanceObserver>>>>>;

use std::fmt::{self};

#[derive(Debug, Clone)]
pub enum ChainError {
    InvalidAddress(String),
    NoReceipt        
}

impl Error for ChainError { }

impl fmt::Display for ChainError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChainError::InvalidAddress(address) => write!(f, "Unable to parse the address: '{address}'"),
            ChainError::NoReceipt => write!(f, "No transaction receipt. Dropped from mempool?") 
        }
    }
}

pub trait TokenData {
    fn symbol(&self) -> String;
    fn identifier(&self) -> String;
    fn decimals(&self) -> u32;
    fn denomiator(&self) -> u128;
}

pub trait BalanceUpdatedObserver {
	fn balance_updated(&mut self, gwei: u128, token: &dyn TokenData);
}

pub trait TokenInteractor {

	fn subscribe(&mut self, observer: Weak<Mutex<BalanceObserver>>);
	fn unsubscribe(&mut self, observer: &Arc<BalanceObserver>);
}