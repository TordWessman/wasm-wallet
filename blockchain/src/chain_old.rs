use std::borrow::Borrow;
use std::sync::Weak;
use std::sync::{RwLock, Arc};

use async_trait::async_trait;
use ethers::abi::Address;
use ethers::prelude::{SignerMiddleware, k256};
use ethers::providers::{Provider, Http};
use ethers::signers::Wallet;

type Client = SignerMiddleware<Provider<Http>, Wallet<k256::ecdsa::SigningKey>>;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub trait BalanceUpdatedObserver {
	fn balance_updated(&self, gwei: u128, token_data: &impl TokenData);
}

#[async_trait]
pub trait TokenInteractor<O> where O: BalanceUpdatedObserver + std::marker::Send {

	async fn update_balance(&self, client: &Client) -> Result<()>;
	fn subscribe(&mut self, observer: Arc<RwLock<O>>);
	fn unsubscribe(&mut self, observer: Arc<RwLock<O>>);
}

pub trait TokenData {
	fn address(&self) -> Address;
	fn symbol(&self) -> Option<String>;
}

impl<O> TokenData for Token<O> where O: BalanceUpdatedObserver + std::marker::Send + ?Sized {
	fn address(&self) -> Address { self.address }
	fn symbol(&self) -> Option<String> { self.symbol.clone() }
}

pub struct Token<O> where O: BalanceUpdatedObserver + std::marker::Send + ?Sized {
	address: Address,
	symbol: Option<String>,
	observers: Vec<Weak<RwLock<O>>>
}

impl<O> Token<O> where O: BalanceUpdatedObserver + std::marker::Send + std::marker::Sync + ?Sized {

	pub fn new(address: Address, symbol: Option<String>) -> Self {
		Self { address: address, symbol: symbol, observers: Vec::new() }
	}

	fn notify_observers(&self, gwei: u128) {
		for observer in self.observers.iter() {
            if let Some(observer_reference) = observer.upgrade() {
                let obs: &RwLock<O> = observer_reference.borrow();
                obs.read().unwrap().balance_updated(gwei, self);
            } else {
				panic!("Implement cleanup");
            }
        }
	}
}

#[async_trait]
impl<O> TokenInteractor<O> for Token<O> where O: BalanceUpdatedObserver + std::marker::Send + std::marker::Sync {

	async fn update_balance(&self, client: &Client) -> Result<()> {

		//panic!("Implmenet!");
		self.notify_observers(42);
		Ok(())
	}

	fn subscribe(&mut self, observer: Arc<RwLock<O>>) {

		let weak_reference = Arc::downgrade(&observer);
		self.observers.push(weak_reference);
	}

	fn unsubscribe(&mut self, observer: Arc<RwLock<O>>) {
		panic!("unsubscribe not implemented");
		//self.observers
	}
}

pub struct Chain<O> where O: BalanceUpdatedObserver + std::marker::Send + std::marker::Sync + ?Sized {
	pub rpc: String,
	pub tokens: Vec<Token<O>>,
	observers: Vec<Weak<RwLock<O>>>
}

impl<O> Chain<O> where O: BalanceUpdatedObserver + std::marker::Send + std::marker::Sync {

	pub fn new(rpc: String) -> Self {
		Self { rpc: rpc, tokens: Vec::new(), observers: Vec::new() }
	}

	pub fn add(self, token: Token<O>) {
		//does not work: self.tokens.push(token);
		//does not work: token.subscribe(Arc::new(RwLock::new(self)));
	}

	pub fn get_tokens(&self) -> &Vec<Token<O>> {
		&self.tokens
	}

	pub fn subscribe(&mut self, observer: Arc<RwLock<O>>) {

		let weak_reference = Arc::downgrade(&observer);
		self.observers.push(weak_reference);
	}

	fn notify_observers(&self, gwei: u128, token_data: &impl TokenData) {
		for observer in self.observers.iter() {
            if let Some(observer_reference) = observer.upgrade() {
                let obs: &RwLock<O> = observer_reference.borrow();
                obs.read().unwrap().balance_updated(gwei, token_data);
            } else {
				panic!("Implement cleanup");
            }
        }
	}
}

impl<O> BalanceUpdatedObserver for Chain<O> where O: BalanceUpdatedObserver + std::marker::Send + std::marker::Sync {

	fn balance_updated(&self, gwei: u128, token_data: &impl TokenData) {

		self.notify_observers(gwei, token_data);
	}
}