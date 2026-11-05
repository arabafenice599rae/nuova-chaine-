//! Unico punto di contatto con l'API di litesvm (pin `=0.18.2`).
//! Aggiornare litesvm significa rileggere solo questo file.

use std::collections::HashMap;

use litesvm::LiteSVM;
use litesvm::types::TransactionResult;
use solana_account::{Account, AccountSharedData, ReadableAccount};
use solana_address::Address;
use solana_clock::Clock;
use solana_hash::Hash;
use solana_transaction::sanitized::{MessageHash, SanitizedTransaction};
use solana_transaction::versioned::VersionedTransaction;

pub use litesvm::types::{FailedTransactionMetadata, SimulatedTransactionInfo, TransactionMetadata};
pub use solana_transaction::TransactionError;

pub struct Svm(LiteSVM);

/// Esito di una transazione eseguita.
pub struct Executed {
    pub result: TransactionResult,
    /// Lamport effettivamente addebitati al pagatore (0 = tx non inclusa).
    pub fee_charged: u64,
}

fn base(lamports_per_signature: u64) -> LiteSVM {
    // Il filtro anti-duplicati e il controllo del blockhash di litesvm sono
    // disattivati: li fa il nodo (exec/mod.rs) con la finestra di N blocchi.
    let mut svm = LiteSVM::default()
        .with_mainnet_features()
        .with_sigverify(false)
        .with_blockhash_check(false)
        .with_transaction_history(0)
        .with_log_bytes_limit(Some(10_000));
    let mut fees = svm.get_fee_structure().clone();
    fees.lamports_per_signature = lamports_per_signature;
    svm.set_fee_structure(fees);
    svm
}

impl Svm {
    /// Stato iniziale costruito con i singoli `with_*` (mai `LiteSVM::new()`).
    pub fn genesis(lamports_per_signature: u64) -> Svm {
        let mut svm = base(lamports_per_signature).with_builtins().with_sysvars();
        // Clock a slot 0 prima di caricare i programmi: litesvm li registra allo
        // slot corrente e lo slot di ogni blocco sarà l'altezza (>= 1).
        svm.set_sysvar(&Clock::default());
        let svm = svm
            .with_feature_accounts()
            .with_default_programs()
            .with_precompiles();
        Svm(svm)
    }

    /// Ricostruisce lo stato da uno snapshot.
    pub fn restore(
        accounts: Vec<(Address, AccountSharedData)>,
        lamports_per_signature: u64,
        latest_blockhash: Hash,
    ) -> Result<Svm, String> {
        let mut svm = base(lamports_per_signature);
        for (k, a) in accounts {
            svm.set_account_no_checks(k, a);
        }
        svm.set_latest_blockhash(latest_blockhash);
        svm.rebuild_caches().map_err(|e| format!("rebuild_caches: {e:?}"))?;
        Ok(Svm(svm))
    }

    pub fn accounts(&self) -> &HashMap<Address, AccountSharedData> {
        &self.0.accounts_db().inner
    }

    pub fn get(&self, k: &Address) -> Option<&AccountSharedData> {
        self.0.accounts_db().get_account_ref(k)
    }

    pub fn lamports(&self, k: &Address) -> u64 {
        self.get(k).map_or(0, |a| a.lamports())
    }

    pub fn set(&mut self, k: Address, a: Account) {
        self.0.set_account(k, a).expect("set_account");
    }

    /// Programma BPF con loader upgradeable e nessuna autorità di upgrade: immutabile.
    pub fn add_program(&mut self, id: Address, elf: &[u8]) -> Result<(), String> {
        self.0.add_program(id, elf).map_err(|e| format!("program {id}: {e:?}"))
    }

    pub fn rent_exempt(&self, len: usize) -> u64 {
        self.0.minimum_balance_for_rent_exemption(len)
    }

    /// Inizio blocco: slot e unix_timestamp nella sysvar Clock, blockhash corrente.
    pub fn begin_block(&mut self, slot: u64, unix_timestamp: i64, blockhash: Hash) {
        let clock = Clock { slot, unix_timestamp, ..Clock::default() };
        self.0.set_sysvar(&clock);
        self.0.set_latest_blockhash(blockhash);
    }

    pub fn execute(&mut self, tx: VersionedTransaction) -> Executed {
        let payer = tx.message.static_account_keys().first().copied().unwrap_or_default();
        let before = self.lamports(&payer);
        let result = self.0.send_transaction(tx);
        let fee_charged = match &result {
            Ok(meta) => meta.fee,
            // In caso di errore l'unico effetto possibile è l'addebito della fee.
            Err(_) => before.saturating_sub(self.lamports(&payer)),
        };
        Executed { result, fee_charged }
    }

    pub fn simulate(
        &self,
        tx: VersionedTransaction,
    ) -> Result<SimulatedTransactionInfo, FailedTransactionMetadata> {
        self.0.simulate_transaction(tx)
    }

    /// Account scrivibili della tx, incluse le address lookup table (v0).
    /// Sovrainsieme degli account che la tx può aver modificato.
    pub fn writable_keys(&self, tx: &VersionedTransaction) -> Vec<Address> {
        let m = &tx.message;
        let fallback = || m.static_account_keys().iter().enumerate()
            .filter(|(i, _)| m.is_maybe_writable_with_reserved_addresses(*i, None::<&std::collections::HashSet<Address>>)).map(|(_, k)| *k).collect();
        let Ok(s) = SanitizedTransaction::try_create(
            tx.clone(), MessageHash::Compute, Some(false), self.0.accounts_db(), &Default::default(),
        ) else {
            return fallback();
        };
        let m = s.message();
        m.account_keys().iter().enumerate().filter(|(i, _)| m.is_writable(*i)).map(|(_, k)| *k).collect()
    }
}
