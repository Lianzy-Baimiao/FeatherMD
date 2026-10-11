//! Two persistent workers: page I/O cannot queue behind a full-file search.
use crate::document::{Document, Page, SearchResult};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
};
pub enum Request {
    Open(PathBuf, u64),
    Page(Arc<Document>, u64, u64),
}
pub enum Reply {
    Opened(u64, Result<(Arc<Document>, Page), String>),
    Page(u64, u64, Result<Page, String>),
    Search(u64, Result<SearchResult, String>),
}
pub struct SearchRequest {
    pub document: Arc<Document>,
    pub query: String,
    pub from: u64,
    pub ticket: u64,
}
pub struct Workers {
    pub io: mpsc::Sender<Request>,
    pub search: mpsc::Sender<SearchRequest>,
    pub replies: mpsc::Receiver<Reply>,
    pub epoch: Arc<AtomicU64>,
}
impl Default for Workers {
    fn default() -> Self {
        Self::new()
    }
}
impl Workers {
    pub fn new() -> Self {
        let (io, rx) = mpsc::channel();
        let (search, sx) = mpsc::channel::<SearchRequest>();
        let (tx, replies) = mpsc::channel();
        let page_tx = tx.clone();
        let epoch = Arc::new(AtomicU64::new(0));
        let search_epoch = epoch.clone();
        thread::spawn(move || {
            while let Ok(mut request) = rx.recv() {
                while let Ok(newest) = rx.try_recv() {
                    request = newest;
                }
                let reply = match request {
                    Request::Open(path, ticket) => Reply::Opened(
                        ticket,
                        Document::open(path)
                            .and_then(|d| {
                                let p = d.page(0)?;
                                Ok((Arc::new(d), p))
                            })
                            .map_err(|e| e.to_string()),
                    ),
                    Request::Page(doc, anchor, ticket) => {
                        Reply::Page(ticket, anchor, doc.page(anchor).map_err(|e| e.to_string()))
                    }
                };
                if page_tx.send(reply).is_err() {
                    break;
                }
            }
        });
        thread::spawn(move || {
            while let Ok(mut r) = sx.recv() {
                while let Ok(newest) = sx.try_recv() {
                    r = newest;
                }
                let result = r
                    .document
                    .search(&r.query, r.from, &search_epoch, r.ticket)
                    .map_err(|e| e.to_string());
                if tx.send(Reply::Search(r.ticket, result)).is_err() {
                    break;
                }
            }
        });
        Self {
            io,
            search,
            replies,
            epoch,
        }
    }
    pub fn cancel_search(&self) -> u64 {
        self.epoch.fetch_add(1, Ordering::Relaxed) + 1
    }
}
impl Drop for Workers {
    fn drop(&mut self) {
        self.cancel_search();
    }
}
