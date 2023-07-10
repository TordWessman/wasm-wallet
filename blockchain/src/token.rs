use std::sync::{Mutex, Arc, Weak};
use ethers::abi::{Address, Abi};
use ethers::prelude::*;

use crate::{shared::*, StringRepresentation};
use crate::chain::*;

abigen!(
    ERC20Token,
    "./ierc20.abi"
);

#[derive(Clone)]
pub struct Token  {
	address: Address,
	symbol: String,
    decimals: u32,
    client: Arc<crate::shared::Client>,
    contract_abi: Abi,
	observers: ObserverList
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Token {} [{}]", self.symbol, self.address.string_representation())
    }
}

impl TokenData for Token {
    fn symbol(&self) -> String { self.symbol.clone() }
    fn decimals(&self) -> u32 { self.decimals }
    fn identifier(&self) -> String { self.address.string_representation() }
    fn denomiator(&self) -> u128 {
        let ten: u128 = 10;
        ten.pow(self.decimals) as u128
    }
}

impl Token {

    pub(crate) fn new(address: Address, 
                      symbol: String, 
                      decimals: u32, 
                      client: Arc<crate::shared::Client>,
                      erc_20_contract_source: String) -> Token {

        Self {  address: address, 
                symbol: symbol, 
                decimals: decimals, 
                client: client,
                contract_abi: serde_json::from_str(&*erc_20_contract_source).expect("Unable to parse ABI"),
                observers: Arc::new(Mutex::new(Vec::new())) }
    }

    fn notify_observers(&self, gwei: u128) {
        println!("Notifying token-observers for gwei: {gwei}");
        let observers = self.observers.lock().unwrap();
        println!("Observer count: {}", observers.len());
        for observer in observers.iter() {

            if let Some(observer) = observer.upgrade() {
                observer.lock().unwrap().balance_updated(gwei, self);
            } else {
                panic!("Observer was nil. Have you cloned the Arc? This should be a cleanup");
            }
        }
    }

    pub fn address(&self) -> Address { self.address }
    
    pub async fn update_balance(&self) -> Result<()> {

        println!("Updating token balance for: {:?}", self);
        
        let contract = Contract::new(self.address, self.contract_abi.clone(), Arc::new(self.client.clone())); 

        let req_method = contract.method::<H160, u128>("balanceOf", self.client.address())?;

        let amount = req_method.call().await?;
        self.notify_observers(amount);
        Ok(())
    }

    pub async fn transfer(&self, to: Address, amount: u64, _from: Option<Address>) -> Result<()> {
        
        let contract = ERC20Token::new(self.address, self.client.clone());
        // println!("------------------ send");
        // print_type_of(&contract);
        
        let _function_call = contract.transfer(to, U256::from(amount));
        println!("  * Function Call: {:?}", _function_call);
        // print_type_of(&_function_call);

        let _pending_tx = _function_call.send().await?;
        println!("  *   Pending transaction: {:?}", _pending_tx);
        // print_type_of(&_pending_tx);
        
        let _transaction_receipt = _pending_tx.await;
        println!("  *   Transaction receipt: {:?}", _transaction_receipt);
        // print_type_of(&_transaction_receipt);
        
        match _transaction_receipt {
            Ok(_) => {
                self.update_balance().await?;
                Ok(())
            },
            Err(error) => Err(Box::new(error))
        }
    }
}

impl TokenInteractor for Token {

	fn subscribe(&mut self, observer: Weak<Mutex<BalanceObserver>>) {
        self.observers.lock().unwrap().push(observer);
    }

	fn unsubscribe(&mut self, _observer: &Arc<BalanceObserver>) {
        panic!("Not implmented!");
    }
}