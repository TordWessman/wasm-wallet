extern crate web_sys;

use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::{panic, sync::Arc, sync::Mutex};
use wasm_bindgen::prelude::*;

use crate::storage::*;
use crate::utils::*;
use crate::wallet::*;
use crate::log;
use blockchain::account::*;
use blockchain::mnemonic;
use blockchain::chain::*;

#[derive(Debug, Clone)]
pub enum ValidationError {
    InvalidPassword(String),
    InvalidUserName(String),
    UserExists,
    InvalidMnemonic
}

impl Error for ValidationError { }

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ValidationError::InvalidPassword(message) => write!(f, "Invalid password: {message}"),
            ValidationError::InvalidUserName(message) => write!(f, "Invalid username: {message}"),
            ValidationError::UserExists => write!(f, "User exists!"),
            ValidationError::InvalidMnemonic => write!(f, "Invalid mnemonic!")
        }
    }
}

#[wasm_bindgen]
pub struct Wazzaaap {
    observer: Arc<Mutex<WalletObserver>>
    
}

#[wasm_bindgen]
impl Wazzaaap {

    pub fn new() -> Self {
        Self { observer: Arc::new(Mutex::new(WalletObserver::new())) }
    }

    fn observer(&self) -> Arc<Mutex<WalletObserver>> { self.observer.clone() }

     pub fn address(&self) -> String {
        self.observer.lock().unwrap().address()
     }

    pub fn chains_serialized(&self) -> String {
        self.observer.lock().unwrap().chains_serialized()
    }

    pub fn balance_for(&self, id: &str) -> f64 {
        self.observer.lock().unwrap().balance_for(id)
    }

    pub fn available_accounts(&self) -> String {
        self.observer.lock().unwrap().available_accounts()
    }

    pub fn account_name(&self) -> String { 
        self.observer.lock().unwrap().account_name()
     }
}

#[wasm_bindgen]
pub struct WalletObserver {
    address: Option<String>,
    chains: Vec<ChainDescriptor>,
    available_accounts: String,
    active_account_name: String,

    /** <id, (gwei, denomiator)> */
    balances: HashMap<String, (f64, f64)>
} 

#[wasm_bindgen]
impl WalletObserver {

    fn new() -> Self {
        Self { address: None, chains: vec![], available_accounts: String::new(), active_account_name: String::new(), balances: HashMap::new() }
    }

    fn available_accounts(&self) -> String { self.available_accounts.clone() }

    pub fn address(&self) -> String {
        match &self.address {
            Some(address) => address.clone(),
            None => "".to_string()
        }
    }

    fn account_name(&self) -> String { self.active_account_name.clone() }

    fn set_account_name(&mut self, account_name: &str) { self.active_account_name = account_name.to_string(); }

    pub fn chains_serialized(&self) -> String {
        return serde_json::to_string(&self.chains).expect("Unable to serialize chains"); 
    }

    #[wasm_bindgen]
    pub fn balance_for(&self, id: &str) -> f64 {

        if let Some(gwei_denomiator) = self.balances.get(&id.to_string()) {
            return gwei_denomiator.0 / gwei_denomiator.1;
        }
        panic!("No balance entry for: {}", id);
    }

    pub fn denomiator_for(&self, id: &str) -> f64 {
        if let Some(gwei_denomiator) = self.balances.get(&id.to_string()) {
            return gwei_denomiator.1;
        }
        panic!("No balance entry for: {}", id);
    }

    fn set_address(&mut self, address: String) {
        self.address = Some(address);
    }

    fn set_chains(&mut self, chains: Vec<ChainDescriptor>) {
        self.chains = chains;
    }

    fn set_available_accounts(&mut self, available_accounts: String) { self.available_accounts = available_accounts; }
}

impl BalanceUpdatedObserver for WalletObserver {

    fn balance_updated(&mut self, gwei: u128, token: &dyn TokenData) {
        self.balances.insert(token.identifier(), (gwei as f64, token.denomiator() as f64));
    }
}

#[wasm_bindgen]
extern {
    pub fn stateChanged(state: SessionState);
    pub fn walletInitialized();
}

#[wasm_bindgen]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionState {
    New = 0,
    Unauthenticated = 1,
    Authenticated = 2
}

type Authentication = KeyChain<DefaultStorage>;

#[wasm_bindgen]
pub struct Session {
    account_name: String,
    state: SessionState,
    storage: Arc<Mutex<DefaultStorage>>,
    keychain: Option<Arc<Mutex<Authentication>>>,
    wallet: Option<Arc<Mutex<Wallet<Authentication>>>>,
    wallet_observer: Arc<Mutex<WalletObserver>>,
    base_url: String
}

const KEY_CURRENTLY_ACTIVE_ACCOUNT_NAME: &str = "KEY_CURRENTLY_ACTIVE_ACCOUNT_NAME";
const KEY_AVAILABLE_ACCOUNT_NAMES: &str = "$KEY_AVAILABLE_ACCOUNT_NAMES";
const DELIMITER: &str = ";";

#[wasm_bindgen]
impl Session {

    pub fn new(wazzaaap: &mut Wazzaaap, base_url: &str) -> Self {
        
        let window = web_sys::window().expect("Unable to load web_sys::window!");
        let storage = DefaultStorage::new(window.local_storage().expect("Unable to get window.local_storage!").unwrap());

        
        let mut state: SessionState = SessionState::New;
        let mut account_name = String::from("");
        if let Some(_account_name) = storage.get(KEY_CURRENTLY_ACTIVE_ACCOUNT_NAME) {
            log!("Using active account: {}", _account_name);
            state = SessionState::Unauthenticated;
            account_name = _account_name;
        }

        if let Some(available_accounts) = storage.get(KEY_AVAILABLE_ACCOUNT_NAMES) {
            wazzaaap.observer.lock().unwrap().set_available_accounts(available_accounts);
        }
        wazzaaap.observer.lock().unwrap().set_account_name(&account_name);

        Self { account_name: account_name,
               state: state,
               storage: Arc::new(Mutex::new(storage)), 
               keychain: None,
               wallet: None,
               wallet_observer: wazzaaap.observer().clone(),
               base_url: base_url.to_string() }
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    pub fn account_name(&self) -> String {
        self.account_name.clone()
    }

    pub fn clear(&mut self) {

        log!("⚠️⚠️⚠️ CLEARING EVERYTHING STORED! ⚠️⚠️⚠️");
        let mut unlocked = self.storage.lock().unwrap();
        let storage: &mut std::sync::MutexGuard<DefaultStorage> = unlocked.borrow_mut();
        storage.clear();
        self.state = SessionState::New;
        stateChanged(self.state);
    }

    pub fn random_mnemonic(&self) -> String {
        mnemonic::random_mnemonic()
    }

    pub async fn sign_in(&mut self, account_name: &str, password: &str) -> bool {

        assert!(matches!(self.state, SessionState::Unauthenticated));
        
        let mut signin_ok: bool = false;

        {
            let storage_guard = self.storage.lock();
            let storage = storage_guard.as_deref().unwrap();
            
            if Authentication::check_password(storage, account_name, password) {
                let keychain = Arc::new(Mutex::new(Authentication::new(self.storage.clone(), password)));
                self.keychain = Some(keychain.clone());
                signin_ok = true;
            } else {
                errorCallback("Invalid credentials");
            }
        }
        if signin_ok {
            self.sign_in_complete(account_name, None).await.expect("Argh! Sign in (log in) completion failed.");
        }
        signin_ok
    }

    pub async fn create_user(&mut self, account_name: &str, password: &str, mnemonic: &str) -> bool {

        assert!(matches!(self.state, SessionState::New));
        
        if let Err(e) = self.validate(account_name, password, mnemonic) {
            errorCallback(&format!("Error: {}", e));
            return false;
        }

        let mut keychain = Authentication::new(self.storage.clone(), password);
        keychain.set(account_name, password);
        let keychain = Arc::new(Mutex::new(keychain));
        self.keychain = Some(keychain.clone());
        
        self.sign_in_complete(account_name, Some(mnemonic)).await.expect("Argh! Sign in completion failed");
        
        true
    }

    async fn sign_in_complete(&mut self, account_name: &str, mnemonic: Option<&str>) -> blockchain::shared::Result<()> {

        self.account_name = account_name.to_string();

        self.create_wallet(account_name, mnemonic);
        
        if let Some(wallet_mutex) = &self.wallet {
           
           let mut wallet = wallet_mutex.lock().unwrap();

           wallet.initialize().await?;

           let mut observer = self.wallet_observer.lock().unwrap();
           observer.set_address(wallet.address());
           log!("denna adressen gäller: {}", wallet.address());
           observer.set_chains(wallet.chains());

        } else {
            panic!("No wallet!");
        }

        self.store_account_name(account_name);
        self.state = SessionState::Authenticated;
        
        stateChanged(self.state);
        walletInitialized();
        Ok(())
        
    }

    pub fn sign_out(&mut self) {

        self.storage.lock().unwrap().delete(KEY_CURRENTLY_ACTIVE_ACCOUNT_NAME);
        self.state = SessionState::New;
        self.account_name = "".to_string();
        self.wallet = None;
        self.keychain = None;

        stateChanged(self.state);
    }

    pub fn prepare_sign_in(&mut self, account_name: &str) {
        self.storage.lock().unwrap().set(KEY_CURRENTLY_ACTIVE_ACCOUNT_NAME, account_name);
        self.account_name = account_name.to_string();
        self.state = SessionState::Unauthenticated;
        self.wallet_observer.lock().unwrap().set_account_name(&account_name);
        stateChanged(self.state);
    }

    fn available_accounts(&self) -> String {
        if let Some(available_accounts_string) = self.storage.lock().unwrap().get(KEY_AVAILABLE_ACCOUNT_NAMES) {
            return available_accounts_string;
        }
        String::new()
    }

    fn validate(&self, account_name: &str, password: &str, mnemonic: &str) -> Result<(), ValidationError> {
        let illegal_characters: Vec<&str> = vec!["$"];

        if account_name.len() <= 2 { return Err(ValidationError::InvalidUserName("Username too short".to_string())); }
        if illegal_characters.iter().filter(|c| account_name.contains(*c)).count() > 0 {
            return Err(ValidationError::InvalidUserName(format!("Username must not contain any of the following characters: {:?}", illegal_characters)));
        }
        if self.available_accounts().split(DELIMITER).filter(|a| *a == account_name).count() > 0 { return Err(ValidationError::UserExists); }

        if password.len() <= 2 { return Err(ValidationError::InvalidPassword("Too short".to_string())); }

        if mnemonic::validate(mnemonic) == false { return Err(ValidationError::InvalidMnemonic); }

        Ok(())

    }

    fn store_account_name(&mut self, account_name: &str) {
        let mut unlocked = self.storage.lock().unwrap();
        let storage: &mut std::sync::MutexGuard<DefaultStorage> = unlocked.borrow_mut();
        storage.set(KEY_CURRENTLY_ACTIVE_ACCOUNT_NAME, account_name);

        let mut available_accounts: Vec<String> = Vec::new();
        if let Some(available_accounts_string) = storage.get(KEY_AVAILABLE_ACCOUNT_NAMES) {
            available_accounts = available_accounts_string.split(DELIMITER).map(String::from).filter(|name| name.len() > 0).collect();
        }
        if available_accounts.contains(&account_name.to_string()) == false {
            available_accounts.push(account_name.to_string());
        }
        let available_accounts_string = available_accounts.join(DELIMITER);
        storage.set(KEY_AVAILABLE_ACCOUNT_NAMES, &available_accounts_string);
        let mut observer = self.wallet_observer.lock().unwrap();
        observer.set_available_accounts(available_accounts_string);
        observer.set_account_name(account_name);
    }

    fn create_wallet(&mut self, account_name: &str, mnemonic: Option<&str>) {

        if let Some(keychain) = &mut self.keychain {

            let weak_observer_reference = Arc::downgrade(&self.wallet_observer);

            let wallet = Wallet::new(
                                self.base_url.clone(),
                                account_name, 
                                keychain.clone(), 
                                self.storage.clone(),
                                Some(weak_observer_reference.clone()));
            
            
            if let Some(mnemonic) = mnemonic {
                let mut kc = keychain.borrow_mut().lock().unwrap();
                kc.save_seed_phrase(&wallet.account_identifier(), mnemonic);
            }
            
            self.wallet = Some(Arc::new(Mutex::new(wallet)));

        } else {
            panic!("Keychain not created!");
        }
    }
}

#[wasm_bindgen]
impl Session {

    pub async fn transfer(&self, id: String, amount: f32, destination: String) -> bool {

        if let Some(wallet_arc) = &self.wallet {
            let wallet = wallet_arc.lock().unwrap();

            let denomiator = self.wallet_observer.lock().unwrap().denomiator_for(&id);
            let amount = ((amount as f64) * denomiator) as u64;
            log!("Will make a transfer of {amount} gwei from chain {id} to {destination}");

            if let Err(error) = wallet.transfer(id.clone(), amount, destination).await {

                errorCallback(&format!("Transfer failed: {}", error));
                return false;
            }
 
            return true;
        }
        
        errorCallback("Wallet not initialized!");
        false
    }

    pub async fn add_chain(&mut self, rpc: &str) -> bool {

        let success: bool;
        let mut chain_id: String = "".to_string();
        if let Some(wallet_arc) = &self.wallet {

            let mut wallet = wallet_arc.lock().unwrap();

            if wallet.stored_rpcs().contains(&rpc.to_string()) {
                errorCallback("Can't add duplicate chain.");
                return false;
            }

            success = match wallet.borrow_mut().add_chain(rpc.to_string()).await {
                Ok(_chain_id) => {
                    chain_id = _chain_id;
                    true
                }
                Err(error) => {
                    errorCallback(&format!("Unable to add chain: {:?}", error));
                    false
                }
            };
            if success {
                log!("Added chain with id: {}", chain_id);
                
                if let Err(error) = wallet.update_balance(chain_id.clone()).await {
                    errorCallback(&format!("Unable fetch balance for chain {}: {:?}", chain_id, error));
                }
            }
           
        } else {
            errorCallback("Wallet not initialized!");
            success = false;
        }

        if success {
            let mut observer = self.wallet_observer.lock().unwrap();
            observer.set_chains(self.chains());
        }
    
        success
    }

    pub async fn add_token(&mut self, chain_id: &str, contract_address: &str, symbol: &str, decimals: u32) -> bool {
        let mut success: bool = false;
        if let Some(wallet_ref) = &self.wallet {
            success = match wallet_ref.lock().unwrap()
                                    .add_token(chain_id.to_string(), contract_address.to_string(), symbol.to_string(), Some(decimals)).await {
                Ok(()) => true,
                Err(error) => {
                    errorCallback(&format!("Unable to add token: {:?}", error));
                    false
                }
            };
        } else {
             errorCallback("Wallet not initialized!");
        }

        if success {
            let mut observer = self.wallet_observer.lock().unwrap();
            observer.set_chains(self.chains());
        }

        success
    }

    fn chains(&self) -> Vec<ChainDescriptor> {
        if let Some(wallet_ref) = &self.wallet {
            let chains = &wallet_ref.lock().unwrap().chains();
            return chains.to_vec();
        }
        panic!("Wallet not initialized");
    }

    fn tokens(&self, chain_id: &str) -> Vec<TokenDescriptor> {
        if let Some(wallet_ref) = &self.wallet {
            let tokens = &wallet_ref.lock().unwrap().tokens(chain_id.to_string());
            return tokens.to_vec();
        }
        panic!("Wallet not initialized");
    }

    pub async fn update_balances(&self) -> bool {

        if let Some(wallet_ref) = &self.wallet {

            match wallet_ref.lock().unwrap().update_balances().await {
                Ok(()) => return true,
                Err(error) => {
                    errorCallback(&format!("Unable to update balances: {:?}", error));
                    return false;
                }
            }
            
        }
        panic!("No wallet!");
    }

}