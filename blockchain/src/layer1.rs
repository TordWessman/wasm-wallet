use std::sync::{Mutex, Arc, Weak};
use std::time::Duration;
use ethers::abi::{Address};
use ethers::prelude::*;
use ethers::providers::{Provider, Http};
use ethers::signers::{LocalWallet};

use crate::ChainsMetadata;
use crate::chain::*;
use crate::shared::*;
use crate::token::*;

#[derive(Clone)]
pub struct Layer1 {
	tokens: Vec<Token>,
    client: Arc<crate::shared::Client>,
    rpc: String,
    decimals: u32,
    symbol: String,
    chain_id: u64,
    erc_20_contract_source: String,
	observers: ObserverList
}

impl std::fmt::Debug for Layer1 {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Chain {} [{}]", self.symbol, self.rpc)
    }
}

impl TokenData for Layer1 {
    fn symbol(&self) -> String { self.symbol.clone() }
    fn decimals(&self) -> u32 { self.decimals }
    fn identifier(&self) -> String { format!("{}", self.chain_id) }
    fn denomiator(&self) -> u128 {
        let ten: u128 = 10;
        ten.pow(self.decimals) as u128
    }
}

const FALLBACK_DECIMAL_COUNT: u64 = 18;
const FALLBACK_SYMBOL_NAME: &str = "<unknown>";

impl Layer1 {

    pub async fn new(rpc: String, 
                     wallet: LocalWallet, 
                     meta_data: &ChainsMetadata, 
                     erc_20_contract_source: String,
                     chain_id: Option<u64>) -> Result<Layer1> {

        let provider = Provider::<Http>::try_from(&rpc)?.interval(Duration::from_millis(10));
        let provider_chain_id = provider.get_chainid().await.expect("couldn't retrieve chain id").as_u64();
        let chain_id = chain_id.unwrap_or(provider_chain_id);
        let wallet = wallet.clone().with_chain_id(chain_id);
        
        let client = SignerMiddleware::new(provider, wallet);
        
        Ok(Self { tokens: Vec::new(), 
                  client: Arc::new(client), 
                  rpc: rpc,
                  decimals: meta_data.get_decimals(chain_id).unwrap_or(FALLBACK_DECIMAL_COUNT) as u32,
                  symbol: meta_data.get_symbol(chain_id).unwrap_or(FALLBACK_SYMBOL_NAME).to_string(),
                  chain_id: chain_id,
                  erc_20_contract_source: erc_20_contract_source,
                  observers: Arc::new(Mutex::new(Vec::new())) 
                })
    }

    /** Add token and return the number of decimals. */
    pub async fn add_token(&mut self, address: String, symbol: String, decimals: Option<u32>) -> Result<u32> {
        
        let decimals = decimals.unwrap_or(self.decimals);
        if let Ok(address) = address.parse::<Address>() {
            let mut token = Token::new(address, symbol, decimals, self.client.clone(), self.erc_20_contract_source.clone());

            let observers = self.observers.lock();
            for observer in observers.unwrap().iter() {
                token.subscribe(observer.to_owned());
            }
            token.update_balance().await?;
            self.tokens.push(token);
            return Ok(decimals);
        }

        Err(Box::new(ChainError::InvalidAddress(address)))
    }

    pub fn tokens(&self) -> &Vec<Token> { &self.tokens }

    pub fn rpc(&self) -> &String { &self.rpc }

    pub async fn update_balance(&self) -> Result<()> {
        let balance = self.client.provider().get_balance(self.client.address(), None).await?;
        self.notify_observers(balance.as_u128());
        Ok(())
    }

    pub async fn transfer(&self, to: Address, amount: u64, from: Option<Address>) -> Result<()> {

        let from: Address = from.unwrap_or(self.client.address());
        println!("Transfer {amount} of {} to {}", self.symbol, to);
        //let nonce1 = self.client.get_transaction_count(from, Some(BlockNumber::Latest.into())).await?;
        let tx = TransactionRequest::new()
            .to(to)
            .from(from)
            .value(amount);
        println!("Sending transaction: {:?}", tx);

        let _pending_tx = self.client.send_transaction(tx, None).await?.confirmations(5);
        println!("Pending transaction: {:?}", _pending_tx);
        let receipt_result = &_pending_tx.await?;

        println!("Receipt Result {:?}", &receipt_result);
        
        if let Some(receipt) = receipt_result {
            let tx = self.client.get_transaction(receipt.transaction_hash).await?;
            println!("Sent tx: {}\n", serde_json::to_string(&tx)?);
            println!("Tx receipt: {}", serde_json::to_string(&receipt)?);
            self.update_balance().await?;
            return Ok(());
        };
        Err(Box::new(ChainError::NoReceipt))
    }

     fn notify_observers(&self, gwei: u128) {
        let observers = self.observers.lock();
        for observer in observers.unwrap().iter() {

            if let Some(observer) = observer.upgrade() {
                observer.lock().unwrap().balance_updated(gwei, self);
            } else {
                panic!("Observer was nil. Have you cloned the Arc? This should be a cleanup");
            }
        }
    }

}

impl TokenInteractor for Layer1 {

	fn subscribe(&mut self, observer: Weak<Mutex<BalanceObserver>>) {
        self.observers.lock().unwrap().push(observer.clone());
        for token in self.tokens.iter_mut() {
            token.subscribe(observer.clone());
        }
    }

	fn unsubscribe(&mut self, _observer: &Arc<BalanceObserver>) {
        panic!("Implement!");
    }
}