use bitcoin::blockdata::transaction::Transaction;
use std::collections::HashMap;
use binary_sv2::ShortTxId;


struct TransacrtionWithHash<'decoder> {
    hash: ShortTxId<'decoder>,
    tx: Transaction,
}

// the transaction in the mempool are
// oredered as fee/weight in descending order
pub struct JDsMempool<'decoder> { 
    mempool: Vec<TransacrtionWithHash<'decoder>>,
}

impl<'decoder> JDsMempool<'decoder> {
    fn new() -> Self {
        JDsMempool{mempool: Vec::new()}
    }
    fn verify_short_id<'a>(&self, tx_short_id: ShortTxId<'a>) -> Option<&Transaction> {
        for transaction_with_hash in self.mempool.iter() {
            if transaction_with_hash.hash == tx_short_id {
                return Some(&transaction_with_hash.tx)
            } else {
                continue
            }
        };
        None
    }
    
    fn order_mempool_by_profitability(mut self) -> JDsMempool<'decoder> {
        self.mempool.sort_by(|a, b| b.tx.get_weight().cmp(&a.tx.get_weight()));
        self
    }
    
    //fn add_transaction_data(mut self, tx_short_id: ShortTxId<'decoder>, transaction: Transaction) -> JDsMempool<'decoder> {
    //    self.mempool.insert(tx_short_id, transaction).unwrap();
    //    self
    //}

    
}
