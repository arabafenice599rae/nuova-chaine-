//! centrifuga-node: tutto ciò che gira nel nodo (esecuzione, journal, snapshot, RPC,
//! stream). CLI e strumenti (wallet, indexer, claims) sono nel crate `centrifuga`.

pub mod archive;
pub mod exec;
pub mod genesis;
pub mod ingress;
pub mod ledger;
pub mod node;
pub mod rpc;
pub mod shadow;
pub mod stream;
