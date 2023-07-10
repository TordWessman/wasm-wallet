//use std::borrow::BorrowMut;
use std::sync::Mutex;
use std::sync::{Arc, Weak};
use ethers::types::Address;
use ethers::utils::hex;

pub mod account;
pub mod chain;
pub mod layer1;
pub mod token;
pub mod chain_metadata;
pub mod shared;
pub mod mnemonic;

use crate::account::*;
use crate::chain::*;
use crate::layer1::*;
use crate::chain_metadata::*;
use crate::shared::*;

pub trait StringRepresentation {
    fn string_representation(&self) -> String;
}

impl StringRepresentation for Address {
    fn string_representation(&self) -> String {
        format!("0x{}", hex::encode(self.as_bytes()))
    }
}

pub struct Portfolio<A> where A: Account {

    chains: Arc<Mutex<Vec<Layer1>>>,
    owner: Arc<Mutex<A>>,
    default_observer: Option<Weak<Mutex<BalanceObserver>>>
}

impl<A> Portfolio<A> where A: Account {

    pub fn new(account: Arc<Mutex<A>>, observer: Option<Weak<Mutex<BalanceObserver>>>) -> Self {
        Self { chains: Arc::new(Mutex::new(Vec::new())), 
               owner: account, 
               default_observer: observer }
    }

    pub fn add_chain(&mut self, chain: Layer1) {
        
        let mut chains = self.chains.lock().unwrap();

        chains.insert(0, chain);

        if let Some(observer_reference) = &self.default_observer {
            chains[0].subscribe(observer_reference.to_owned());
        }
    }

    pub async fn update_balances(&self) -> Result<()> {

        let chains = self.chains.lock().unwrap();
        for chain in chains.iter() {
            
            chain.update_balance().await?;

            for token in chain.tokens().iter() {
                token.update_balance().await?;
            }
        }
        Ok(())
    }

    pub fn chains(&self) -> Arc<Mutex<Vec<Layer1>>> {
        self.chains.clone()
    }

    pub fn owner(&self) -> Arc<Mutex<A>> {
        self.owner.clone()
    }

}



