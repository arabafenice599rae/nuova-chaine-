//! Programma di prova per `solana program deploy` (solo test, mai nel genesis).
//! Ogni chiamata scrive la versione nei log e nei return data: v1, o v2 con la
//! feature `v2`, per verificare l'aggiornamento.

use pinocchio::account_info::AccountInfo;
use pinocchio::pubkey::Pubkey;
use pinocchio::ProgramResult;

#[cfg(target_os = "solana")]
pinocchio::entrypoint!(process);

#[cfg(not(feature = "v2"))]
const VERSION: &str = "echo v1";
#[cfg(feature = "v2")]
const VERSION: &str = "echo v2";

pub fn process(_program: &Pubkey, _accounts: &[AccountInfo], _data: &[u8]) -> ProgramResult {
    pinocchio::msg!(VERSION);
    pinocchio::program::set_return_data(VERSION.as_bytes());
    Ok(())
}
