use blockchain::account::*;
use blockchain::chain_metadata::*;
use blockchain::chain::*;
use blockchain::layer1::Layer1;
use serde::{Deserialize, Serialize};
use crate::storage::*;
use blockchain::*;
use crate::log;

use std::error::Error;
use std::fmt;
use std::sync::Weak;
use std::{sync::Arc, sync::Mutex};

#[derive(Debug, Clone)]
pub enum WalletError {
    NotInitialized,
    ChainNotFound(String)
}


impl Error for WalletError { }

impl fmt::Display for WalletError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WalletError::NotInitialized => write!(f, "Wallet not initialized!"),
            WalletError::ChainNotFound(chain_id) => write!(f, "Chain id {} not found", chain_id) 
        }
    }
}

const ERC_20_CONTRACT_FILE_NAME: &str = "/ierc20.abi"; 
const CHAINS_METADATA_URL: &str = "https://chainid.network/chains.json";
const KEY_SEED_PHRASE_POSTFIX: &str = "_$SEED_PHRASE";
const KEY_RPCS_POSTFIX: &str = "_$RPCS";
const KEY_TOKENS_POSTFIX: &str = "_$TOKENS";
const DELIMITER: &str = ";";

impl<T> KeyChain<T> where T: Storage {

    fn get_seed_phase_key(&self, account_id: &str) -> String {
        format!("{}{}", account_id, KEY_SEED_PHRASE_POSTFIX)
    }
}

impl<T> Credentials for KeyChain<T> where T: Storage {

    fn get_seed_phrase(&self, account_identifier: &AccountIdentifier) -> Option<String> {
        return self.get(&self.get_seed_phase_key(&account_identifier.id)).to_owned()
    }

    fn save_seed_phrase(&mut self, account_identifier: &AccountIdentifier, seed_phrase: &str) {
        let identifier = &self.get_seed_phase_key(&account_identifier.id);
        self.set(&String::from(identifier), &seed_phrase);
    }
}

pub trait WalletRepresentation {
    fn get_address(&self) -> String;
}

pub struct Wallet<C> where C: Credentials {

    local_base_url: String,
    account_identifier: AccountIdentifier,
    account: Arc<Mutex<SigningAccount<C>>>,
    storage: Arc<Mutex<dyn Storage>>,
    portfolio: Arc<Mutex<Portfolio<SigningAccount<C>>>>,
    chains_metadata: ChainsMetadata,
    erc_20_contract_source: Option<String>
    
}

fn rpcs_key(account_identifier: &AccountIdentifier) -> String {
    format!("{}{}", account_identifier.id, KEY_RPCS_POSTFIX)
}

fn tokens_key(account_identifier: &AccountIdentifier, chain_id: &str) -> String {
    format!("{}_{}{}", account_identifier.id, chain_id, KEY_TOKENS_POSTFIX)
}

unsafe impl<C> Send for Wallet<C> where C: Credentials + Send { }

impl<C> Wallet<C> where C: Credentials + std::fmt::Debug {

    pub fn new(
            local_base_url: String,
            account_name: &str, 
            keychain: Arc<Mutex<C>>,
            storage: Arc<Mutex<dyn Storage>>,
            observer: Option<Weak<Mutex<BalanceObserver>>>) -> Self {
        let account_identifier = AccountIdentifier{ id: account_name.to_string() };

        let chains_metadata = ChainsMetadata::new(CHAINS_METADATA_URL.to_string());
        let account = Arc::new(Mutex::new(SigningAccount::new(&account_identifier, keychain.clone())));
        let portfolio = Arc::new(Mutex::new(Portfolio::new(account.clone(), observer)));
            
            Self { local_base_url: local_base_url,
                   account_identifier: account_identifier,
                   account: account, 
                   portfolio: portfolio,
                   storage: storage,
                   chains_metadata: chains_metadata,
                   erc_20_contract_source: None }
    }

    pub fn account_identifier(&self) -> AccountIdentifier {
        self.account_identifier.clone()
    }

    pub(crate) async fn initialize(&mut self) -> blockchain::shared::Result<()> {
        log!("Initializing wallet.");
        if self.chains_metadata.empty() {
            self.chains_metadata.download().await?;
        }
        
        if self.erc_20_contract_source.is_none() {
            let url: String=  format!("{}{}", &self.local_base_url,ERC_20_CONTRACT_FILE_NAME);
            self.erc_20_contract_source = Some(reqwest::get(&url).await?.text().await?);
        }
        
        self.account.lock().unwrap().create_wallet();
        
        self.load_coins_to_portfolio().await?;
        self.update_balances().await?;

        log!("Wallet initialized.");
        Ok(())
    }

    pub fn address(&self) -> String {
        if let Some(address) = self.account.lock().unwrap().address() {
            return address.string_representation();
        }
        "".to_string()
    }

    pub async fn add_chain(&mut self, rpc: String) -> blockchain::shared::Result<String> {
        let chain_id = self.add_chain_to_portfolio(rpc.clone()).await?;
        self.store_rpc(rpc.clone());

        Ok(chain_id)
    }

    pub async fn add_token(&mut self, chain_id: String, contract_address: String, symbol: String, decimals: Option<u32>) -> blockchain::shared::Result<()> {
        
        let decimals = self.add_token_to_portfolio(chain_id.clone(), contract_address.clone(), symbol.clone(), decimals).await?;
        let token_descriptor = TokenDescriptor { contract_address: contract_address, symbol: symbol, decimals: decimals };
        self.store_token(chain_id, token_descriptor)?;
        Ok(())
    }

    pub async fn update_balances(&self) -> blockchain::shared::Result<()> {
        self.portfolio.lock().unwrap().update_balances().await
    }

    pub async fn update_balance(&self, chain_id: String) -> blockchain::shared::Result<()> {
        let portfolio_chains = self.portfolio.lock().unwrap().chains();
        for chain in portfolio_chains.lock().unwrap().iter() {
            if chain.identifier() == chain_id {
                chain.update_balance().await?;
            }
        }

        Ok(())
    }

    pub fn chains(&self) -> Vec<ChainDescriptor> {
        let mut chains = Vec::<ChainDescriptor>::new();
        let portfolio_chains = self.portfolio.lock().unwrap().chains();
        for chain in portfolio_chains.lock().unwrap().iter() {
            let mut tokens: Vec<TokenDescriptor> = Vec::new();
            for token in chain.tokens().iter() {
                tokens.push(TokenDescriptor { 
                    contract_address: token.address().string_representation(), 
                    symbol: token.symbol(), 
                    decimals: token.decimals() });
            }

            chains.push(ChainDescriptor { 
                id: chain.identifier(), 
                symbol: chain.symbol(), 
                rpc: chain.rpc().to_string(), 
                decimals: chain.decimals(),
                tokens: tokens });
        }
        chains
    }

    pub fn tokens(&self, chain_id: String) -> Vec<TokenDescriptor> {
        for chain in self.chains() {
            if chain.id == chain_id {
                return chain.tokens.clone();
            }
        }
        vec![]
    }

    /** Transfer from chain (id) or token (address). */
    pub async fn transfer(&self, id: String, amount: u64, destination: String) -> blockchain::shared::Result<()> {
        let chains = self.portfolio.lock().unwrap().chains();
        let chains = chains.lock().unwrap();
        if let Some(chain) = chains.iter().find(|c| c.identifier() == id) {
            chain.transfer(destination.parse().unwrap(), amount, None).await?;
            return Ok(());
        }
        else if let Some(token) = chains.iter().map(|c| c.tokens()).flat_map(|t| t).find(|t| t.identifier() == id) {
            token.transfer(destination.parse().unwrap(), amount, None).await?;
            return Ok(());
        }
        //else if let Some(token) = chains.iter().map(|c| c.tokens())//.find(|c| c.tokens().iter().f)

        Err(Box::new(WalletError::ChainNotFound(id)))
    }

}

impl<C> Wallet<C> where C: Credentials + std::fmt::Debug {

    async fn load_coins_to_portfolio(&mut self) -> blockchain::shared::Result<()> {
        let mut chain_ids = Vec::<String>::new();
        for rpc in self.stored_rpcs().iter() {
            log!("Adding |{}| to portfolio", rpc);
            chain_ids.push(self.add_chain_to_portfolio(rpc.to_owned()).await?);
        }

        for chain_id in chain_ids.iter() {
            for token_descriptor in self.stored_tokens(chain_id.clone()).iter() {
                self.add_token_to_portfolio(
                    chain_id.clone(), 
                    token_descriptor.contract_address.clone(), 
                    token_descriptor.symbol.clone(), 
                    Some(token_descriptor.decimals)).await?;
            }
        }

        Ok(())
    }

    async fn add_chain_to_portfolio(&mut self, rpc: String) -> blockchain::shared::Result<String> {

        match self.account.lock().unwrap().wallet() {
            Some(wallet) => {
                let chain = Layer1::new(
                    rpc.clone(), 
                    wallet, 
                    &self.chains_metadata, 
                    self.erc_20_contract_source.clone().unwrap(),
                    None).await?;
                let chain_id = chain.identifier();
                self.portfolio.lock().unwrap().add_chain(chain);
                Ok(chain_id)
            },
            None => Err(Box::new(WalletError::NotInitialized))
        }
    }

    async fn add_token_to_portfolio(&mut self, chain_id: String, address: String, symbol: String, decimals: Option<u32>) -> blockchain::shared::Result<u32> {
        let portfolio_chains = self.portfolio.lock().unwrap().chains();
        let mut chains = portfolio_chains.lock().unwrap();
        for chain in chains.iter_mut() {
            if chain.identifier() == chain_id {
                return Ok(chain.add_token(address.clone(), symbol.clone(), decimals).await?);
            }
        }
        Err(Box::new(WalletError::ChainNotFound(chain_id)))
    }

    pub fn stored_rpcs(&self) -> Vec<String> {
        let storage = self.storage.lock().unwrap();
        let rpcs_string = storage.get(&rpcs_key(&self.account_identifier)).unwrap_or("".to_string());
        rpcs_string.split(DELIMITER).map(String::from).filter(|rpc| rpc.len() > 0).collect()
    }

    pub fn stored_tokens(&self, chain_id: String) -> Vec<TokenDescriptor> {
        let storage = self.storage.lock().unwrap();
        if let Some(tokens_serialized) = storage.get(&tokens_key(&self.account_identifier, &chain_id)) {

            return serde_json::from_str(&tokens_serialized).unwrap(); 
        }
        vec![]
    }

    fn store_rpc(&mut self, rpc: String) {
        let mut rpcs = self.stored_rpcs();
        let mut storage = self.storage.lock().unwrap();
        rpcs.push(rpc.clone());
        let rpcs_string = rpcs.join(DELIMITER);
        storage.set(&rpcs_key(&self.account_identifier), &rpcs_string);
    }

    fn store_token(&mut self, chain_id: String, token_descriptor: TokenDescriptor) -> blockchain::shared::Result<()> {
        let mut tokens = self.stored_tokens(chain_id.clone());
        let mut storage = self.storage.lock().unwrap();
        tokens.push(token_descriptor.clone());
        let tokens_string = serde_json::to_string(&tokens)?;
        storage.set(&tokens_key(&self.account_identifier, &chain_id), &tokens_string);
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChainDescriptor {
    pub id: String,
    pub symbol: String,
    pub rpc: String,
    pub decimals: u32,
    pub tokens: Vec<TokenDescriptor>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenDescriptor {
    pub contract_address: String,
    pub symbol: String,
    pub decimals: u32
}

impl TokenData for TokenDescriptor {
    fn symbol(&self) -> String { self.symbol.clone() }
    fn identifier(&self) -> String { self.contract_address.clone() }
    fn decimals(&self) -> u32 { self.decimals }
    fn denomiator(&self) -> u128 {
        let ten: u128 = 10;
        ten.pow(self.decimals) as u128
    }
}

impl TokenData for ChainDescriptor {
    fn symbol(&self) -> String { self.symbol.clone() }
    fn identifier(&self) -> String { self.id.clone() }
    fn decimals(&self) -> u32 { self.decimals }
    fn denomiator(&self) -> u128 {
        let ten: u128 = 10;
        ten.pow(self.decimals) as u128
    }
}
