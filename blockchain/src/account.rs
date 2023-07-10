use ethers::signers::{LocalWallet, MnemonicBuilder, Signer /*, Signer */};
use ethers::prelude::coins_bip39::English;
use ethers::types::Address;

use std::borrow::BorrowMut;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
//use ethers::prelude::*;
//use crate::chain2::*;

pub trait Credentials {

    fn get_seed_phrase(&self, account_identifier: &AccountIdentifier) -> Option<String>;
    fn save_seed_phrase(&mut self, account_identifier: &AccountIdentifier, seed_phrase: &str);
}

pub trait Account {

    fn identifier(&self) -> AccountIdentifier;
    fn wallet(&self) -> Option<LocalWallet>;
    fn create_wallet(&mut self);
    fn address(&self) -> Option<Address>;
}

impl std::fmt::Debug for dyn Account {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Account [{:?}]", self.identifier())
    }
}


#[derive(Debug)]
pub struct AccountIdentifier {
    pub id: String
}

impl Clone for AccountIdentifier {
 
    fn clone(&self) -> Self {
        AccountIdentifier{ id: self.id.to_owned() }
    }
}

#[derive(Debug)]
pub struct SigningAccount<C> where C: Credentials {
    credentials: Arc<Mutex<C>>,
    wallets: Arc<Mutex<Vec<LocalWallet>>>,
    identifier: AccountIdentifier
}


impl<C> SigningAccount<C> where C: Credentials {

    pub fn new(identifier: &AccountIdentifier, credentials: Arc<Mutex<C>>) -> Self {

        Self { credentials: credentials, identifier: identifier.clone(), wallets: Arc::new(Mutex::new(Vec::new())) }
    }

    fn create_wallet(identifier: &AccountIdentifier, credentials: Arc<Mutex<C>>) -> LocalWallet {
        match credentials.lock().unwrap().get_seed_phrase(identifier) {
            Some(seed_phrase) => {
                println!("Seed phrase: {seed_phrase}");
                let compiled = MnemonicBuilder::<English>::default().phrase(seed_phrase.as_str()).build();
                if let Err(error) = compiled {
                    panic!("Invalid seed phrase: {seed_phrase}. Error: {error}");
                }
                let mnemonic = compiled.unwrap();
                let local_wallet = LocalWallet::from(mnemonic);
                local_wallet
            },
            None => panic!("No seed phrase set")
        }
    }

}

impl<C> Account for SigningAccount<C> where C: Credentials {

    fn identifier(&self) -> AccountIdentifier {
        self.identifier.clone()
    }

    fn address(&self) -> Option<Address> {
        let wallets = self.wallets.lock().unwrap();
        if let Some(wallet) = wallets.first() {
            return Some(wallet.address());
        }
        //panic!("Got no wallet: {}", wallets.len());
        None
    }

    fn create_wallet(&mut self) {
        //TODO: Support for multiple wallets
        let wallet = SigningAccount::<C>::create_wallet(
            &self.identifier,
            self.credentials.clone());
        
        self.wallets.lock().unwrap().borrow_mut().push(wallet); //.insert(0, wallet);
    }

    fn wallet(&self) -> Option<LocalWallet> {
        match self.wallets.lock() {
            Ok(apa) => {
                let hund = Arc::new(apa.to_owned());
                if let Some(wallet) = hund.first() {
                    return Some(wallet.clone());
                }
            },
            Err(error) => {
                panic!("Unable to lock wallets {}", error);
            }
        }; 
        
        None
    }
}