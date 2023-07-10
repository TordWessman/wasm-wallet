use ethers::prelude::coins_bip39::*;
use rand::rngs::OsRng;

pub fn validate(mnemonic: &str) -> bool {    
    if let Ok(_) = Mnemonic::<English>::new_from_phrase(mnemonic) {
        return true;
    }
    false
}

pub fn random_mnemonic() -> String {
    let mut rng = OsRng;
    let mnemonic: Mnemonic<English> = Mnemonic::new(&mut rng);
    mnemonic.to_phrase()
}