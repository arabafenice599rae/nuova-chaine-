//! Programma timelock di centrifuga (Pinocchio, nessuna dipendenza transitiva).
//!
//! Vault (creati nel genesis, mai dal programma), layout di `VAULT_LEN` byte:
//!   [0] tag=1  [1] tipo  [2..34] beneficiario  [34..66] autorità
//!   [66..74] totale  [74..82] inizio  [82..90] fine  [90..98] prelevato
//! Tipi: OPERATOR (withdraw lineare tra inizio e fine), AIRDROP (claim fino a
//! fine esclusa, poi to_reserve), RESERVE (solo claim, da inizio = sblocco),
//! MINING (nessuna istruzione: lo muove solo il nodo con le regole di fee).
//!
//! Ondata (account creato dall'autorità, posseduto dal programma):
//!   [0] tag=2  [1..33] vault  [33..41] importo per claim  [41..] bitmap dei claim
//!
//! Istruzioni (primo byte):
//!   0 withdraw                [vault(w), beneficiario(w)]
//!   1 init_wave(importo u64)  [ondata(w), vault, autorità(s)]
//!   2 claim(indice u32)       [vault(w), ondata(w), destinatario(w), sysvar istruzioni]
//!   3 to_reserve              [airdrop(w), reserve(w)]  chiunque, da fine di airdrop
//!
//! Il claim richiede, subito prima, un'istruzione della precompile ed25519 con la
//! firma dell'autorità su `claim_message(ondata, destinatario, importo, indice)`.
//! I lamport escono solo se resta il minimo per la rent exemption: il limite è il
//! saldo reale del vault, non un totale fisso.

use pinocchio::ProgramResult;
use pinocchio::account_info::AccountInfo;
use pinocchio::program_error::ProgramError;
use pinocchio::pubkey::Pubkey;
use pinocchio::sysvars::Sysvar;
use pinocchio::sysvars::clock::Clock;
use pinocchio::sysvars::instructions::Instructions;
use pinocchio::sysvars::rent::Rent;

#[cfg(target_os = "solana")]
pinocchio::entrypoint!(process);

pub const VAULT_TAG: u8 = 1;
pub const WAVE_TAG: u8 = 2;
pub const VAULT_LEN: usize = 98;
pub const WAVE_HEADER: usize = 41;

pub const OPERATOR: u8 = 1;
pub const AIRDROP: u8 = 2;
pub const MINING: u8 = 3;
pub const RESERVE: u8 = 4;

/// Ed25519SigVerify111111111111111111111111111
const ED25519_PROGRAM: Pubkey =
    [3, 125, 70, 214, 124, 147, 251, 190, 18, 249, 66, 143, 131, 141, 64, 255, 5, 112, 116, 73, 39, 244, 138, 100, 252, 202, 112, 68, 128, 0, 0, 0];

/// Errori propri (ProgramError::Custom).
#[repr(u32)]
pub enum Error {
    WrongAccount = 1,
    WrongKind = 2,
    NotAuthority = 3,
    Locked = 4,
    AlreadyClaimed = 5,
    BadSignature = 6,
    InsufficientBalance = 7,
    BadIndex = 8,
}

impl From<Error> for ProgramError {
    fn from(e: Error) -> Self {
        ProgramError::Custom(e as u32)
    }
}

/// Messaggio firmato dall'autorità per un claim: ondata ‖ destinatario ‖ importo ‖ indice.
pub fn claim_message(wave: &Pubkey, claimant: &Pubkey, amount: u64, index: u32) -> [u8; 76] {
    let mut m = [0u8; 76];
    m[..32].copy_from_slice(wave);
    m[32..64].copy_from_slice(claimant);
    m[64..72].copy_from_slice(&amount.to_le_bytes());
    m[72..76].copy_from_slice(&index.to_le_bytes());
    m
}

fn u64_at(d: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(d[o..o + 8].try_into().unwrap())
}

/// Lamport (prelevabili) dopo aver lasciato il minimo per la rent exemption.
fn spendable(a: &AccountInfo) -> Result<u64, ProgramError> {
    Ok(a.lamports().saturating_sub(Rent::get()?.minimum_balance(a.data_len())))
}

fn move_lamports(from: &AccountInfo, to: &AccountInfo, amount: u64) -> ProgramResult {
    if amount > spendable(from)? {
        return Err(Error::InsufficientBalance.into());
    }
    *from.try_borrow_mut_lamports()? -= amount;
    *to.try_borrow_mut_lamports()? += amount;
    Ok(())
}

/// Vault posseduto dal programma e del tipo richiesto; restituisce il tipo.
fn vault_kind(v: &AccountInfo, program: &Pubkey, kinds: &[u8]) -> Result<u8, ProgramError> {
    let d = v.try_borrow_data()?;
    if !v.is_owned_by(program) || d.len() != VAULT_LEN || d[0] != VAULT_TAG {
        return Err(Error::WrongAccount.into());
    }
    if !kinds.contains(&d[1]) {
        return Err(Error::WrongKind.into());
    }
    Ok(d[1])
}

fn check_authority(v: &AccountInfo, auth: &AccountInfo) -> ProgramResult {
    if !auth.is_signer() || v.try_borrow_data()?[34..66] != auth.key()[..] {
        return Err(Error::NotAuthority.into());
    }
    Ok(())
}

pub fn process(program: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let (tag, args) = data.split_first().ok_or(ProgramError::InvalidInstructionData)?;
    let arg_u64 = || args.get(..8).map(|b| u64::from_le_bytes(b.try_into().unwrap())).ok_or(ProgramError::InvalidInstructionData);
    match (tag, accounts) {
        (0, [vault, beneficiary, ..]) => withdraw(program, vault, beneficiary),
        (1, [wave, vault, auth, ..]) => init_wave(program, wave, vault, auth, arg_u64()?),
        (2, [vault, wave, claimant, ixs, ..]) => {
            let index = args.get(..4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or(ProgramError::InvalidInstructionData)?;
            claim(program, vault, wave, claimant, ixs, index)
        }
        (3, [airdrop, reserve, ..]) => to_reserve(program, airdrop, reserve),
        (0..=3, _) => Err(ProgramError::NotEnoughAccountKeys),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

/// Solo il vault dell'operatore: sblocco lineare per altezza tra inizio e fine.
/// Chiunque può chiamarlo; i lamport vanno sempre al beneficiario registrato.
fn withdraw(program: &Pubkey, vault: &AccountInfo, beneficiary: &AccountInfo) -> ProgramResult {
    vault_kind(vault, program, &[OPERATOR])?;
    let height = Clock::get()?.slot;
    let amount = {
        let d = vault.try_borrow_data()?;
        if d[2..34] != beneficiary.key()[..] {
            return Err(Error::WrongAccount.into());
        }
        let (total, start, end, done) = (u64_at(&d, 66), u64_at(&d, 74), u64_at(&d, 82), u64_at(&d, 90));
        let unlocked = if height >= end {
            total
        } else if height <= start {
            0
        } else {
            (total as u128 * (height - start) as u128 / (end - start) as u128) as u64
        };
        unlocked.saturating_sub(done)
    };
    move_lamports(vault, beneficiary, amount)?;
    let mut d = vault.try_borrow_mut_data()?;
    let done = u64_at(&d, 90) + amount;
    d[90..98].copy_from_slice(&done.to_le_bytes());
    Ok(())
}

/// Inizializza un'ondata di claim a importo fisso su un vault airdrop o reserve.
/// L'account è creato in precedenza (system create_account) con owner il programma.
fn init_wave(program: &Pubkey, wave: &AccountInfo, vault: &AccountInfo, auth: &AccountInfo, amount: u64) -> ProgramResult {
    vault_kind(vault, program, &[AIRDROP, RESERVE])?;
    check_authority(vault, auth)?;
    let mut d = wave.try_borrow_mut_data()?;
    // Tag 0: mai inizializzata (create_account azzera i dati).
    if !wave.is_owned_by(program) || d.len() <= WAVE_HEADER || d[0] != 0 || amount == 0 {
        return Err(Error::WrongAccount.into());
    }
    d[0] = WAVE_TAG;
    d[1..33].copy_from_slice(vault.key());
    d[33..41].copy_from_slice(&amount.to_le_bytes());
    Ok(())
}

fn claim(program: &Pubkey, vault: &AccountInfo, wave: &AccountInfo, claimant: &AccountInfo, ixs: &AccountInfo, index: u32) -> ProgramResult {
    let kind = vault_kind(vault, program, &[AIRDROP, RESERVE])?;
    let (authority, start, end) = {
        let v = vault.try_borrow_data()?;
        (<[u8; 32]>::try_from(&v[34..66]).unwrap(), u64_at(&v, 74), u64_at(&v, 82))
    };
    // Reserve: nessun claim prima dello sblocco. Airdrop: nessun claim dalla scadenza.
    let height = Clock::get()?.slot;
    if height < start || (kind == AIRDROP && height >= end) {
        return Err(Error::Locked.into());
    }
    let amount = {
        let w = wave.try_borrow_data()?;
        if !wave.is_owned_by(program) || w.len() <= WAVE_HEADER || w[0] != WAVE_TAG || w[1..33] != vault.key()[..] {
            return Err(Error::WrongAccount.into());
        }
        u64_at(&w, 33)
    };
    let (byte, bit) = (WAVE_HEADER + index as usize / 8, 1u8 << (index % 8));
    if byte >= wave.data_len() {
        return Err(Error::BadIndex.into());
    }
    if wave.try_borrow_data()?[byte] & bit != 0 {
        return Err(Error::AlreadyClaimed.into());
    }
    verify_signature(ixs, &authority, &claim_message(wave.key(), claimant.key(), amount, index))?;
    move_lamports(vault, claimant, amount)?;
    wave.try_borrow_mut_data()?[byte] |= bit;
    Ok(())
}

/// Dalla scadenza dei claim (fine del vault airdrop) chiunque può spostare
/// tutto il saldo residuo di "airdrop" in "reserve". Nessun importo, nessuna firma.
fn to_reserve(program: &Pubkey, airdrop: &AccountInfo, reserve: &AccountInfo) -> ProgramResult {
    vault_kind(airdrop, program, &[AIRDROP])?;
    vault_kind(reserve, program, &[RESERVE])?;
    if Clock::get()?.slot < u64_at(&airdrop.try_borrow_data()?, 82) {
        return Err(Error::Locked.into());
    }
    move_lamports(airdrop, reserve, spendable(airdrop)?)
}

/// L'istruzione precedente deve essere la precompile ed25519 con una sola firma
/// di `authority` su `msg`, con firma, chiave e messaggio nella stessa istruzione.
/// La precompile ha già verificato la firma (altrimenti la tx fallisce).
fn verify_signature(ixs: &AccountInfo, authority: &Pubkey, msg: &[u8]) -> ProgramResult {
    let ixs = Instructions::try_from(ixs)?;
    let ix = ixs.get_instruction_relative(-1).map_err(|_| Error::BadSignature)?;
    let d = ix.get_instruction_data();
    if ix.get_program_id() != &ED25519_PROGRAM || d.len() < 16 || d[0] != 1 {
        return Err(Error::BadSignature.into());
    }
    let u16_at = |o: usize| u16::from_le_bytes([d[o], d[o + 1]]) as usize;
    // Offsets: firma, ix firma, chiave, ix chiave, messaggio, lunghezza, ix messaggio.
    let (pk, msg_off, msg_len) = (u16_at(6), u16_at(10), u16_at(12));
    let same_ix = [4, 8, 14].iter().all(|o| u16_at(*o) == u16::MAX as usize);
    let ok = same_ix && d.get(pk..pk + 32) == Some(&authority[..]) && msg_len == msg.len() && d.get(msg_off..msg_off + msg_len) == Some(msg);
    if ok { Ok(()) } else { Err(Error::BadSignature.into()) }
}
