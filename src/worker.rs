//! Independent page, search, find-all and outline workers keep navigation responsive.
use crate::{
    document::{Document, Matches, Page, Query, SearchResult},
    outline::{self, Outline},
};
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
    Outline(u64, Result<Outline, String>),
    Found(u64, Result<Matches, String>),
}
pub struct SearchRequest {
    pub document: Arc<Document>,
    pub query: String,
    pub match_case: bool,
    pub from: u64,
    pub ticket: u64,
}
/// Every match of a query, for the results list.
pub struct FindRequest {
    pub document: Arc<Document>,
    pub query: String,
    pub match_case: bool,
    pub ticket: u64,
    /// Bytes scanned so far, for the list's progress line.
    pub progress: Arc<AtomicU64>,
}
pub struct OutlineRequest {
    pub document: Arc<Document>,
    pub ticket: u64,
    /// Bytes scanned so far, for the reader's progress line.
    pub progress: Arc<AtomicU64>,
}
pub struct Workers {
    pub io: mpsc::Sender<Request>,
    pub search: mpsc::Sender<SearchRequest>,
    pub outline: mpsc::Sender<OutlineRequest>,
    pub find: mpsc::Sender<FindRequest>,
    pub replies: mpsc::Receiver<Reply>,
    pub epoch: Arc<AtomicU64>,
    pub outline_epoch: Arc<AtomicU64>,
    pub find_epoch: Arc<AtomicU64>,
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
        let (outline, ox) = mpsc::channel::<OutlineRequest>();
        let (find, fx) = mpsc::channel::<FindRequest>();
        let (tx, replies) = mpsc::channel();
        let page_tx = tx.clone();
        let outline_tx = tx.clone();
        let find_tx = tx.clone();
        let find_epoch = Arc::new(AtomicU64::new(0));
        let worker_find_epoch = find_epoch.clone();
        let epoch = Arc::new(AtomicU64::new(0));
        let search_epoch = epoch.clone();
        let outline_epoch = Arc::new(AtomicU64::new(0));
        let worker_outline_epoch = outline_epoch.clone();
        thread::spawn(move || {
            while let Ok(mut request) = rx.recv() {
                while let Ok(newest) = rx.try_recv() {
                    request = newest;
                }
                let reply = match request {
                    Request::Open(path, ticket) => Reply::Opened(
                        ticket,
                        Document::open(path)
                            .and_then(|mut d| {
                                let p = d.page(0)?;
                                // Plain text that starts like a log is read as one.
                                d.format = crate::plain::sniff(d.format, &p.lines);
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
                    .search(
                        Query {
                            text: &r.query,
                            match_case: r.match_case,
                        },
                        r.from,
                        &search_epoch,
                        r.ticket,
                    )
                    .map_err(|e| e.to_string());
                if tx.send(Reply::Search(r.ticket, result)).is_err() {
                    break;
                }
            }
        });
        thread::spawn(move || {
            while let Ok(mut request) = ox.recv() {
                while let Ok(newest) = ox.try_recv() {
                    request = newest;
                }
                let result = match outline::scan(
                    &request.document,
                    &worker_outline_epoch,
                    request.ticket,
                    &request.progress,
                ) {
                    Ok(Some(outline)) => Ok(outline),
                    Ok(None) => continue,
                    Err(error) => Err(error.to_string()),
                };
                if worker_outline_epoch.load(Ordering::Relaxed) == request.ticket
                    && outline_tx
                        .send(Reply::Outline(request.ticket, result))
                        .is_err()
                {
                    break;
                }
            }
        });
        thread::spawn(move || {
            while let Ok(mut request) = fx.recv() {
                while let Ok(newest) = fx.try_recv() {
                    request = newest;
                }
                let result = match request.document.find_all(
                    Query {
                        text: &request.query,
                        match_case: request.match_case,
                    },
                    &worker_find_epoch,
                    request.ticket,
                    &request.progress,
                ) {
                    Ok(Some(matches)) => Ok(matches),
                    Ok(None) => continue,
                    Err(error) => Err(error.to_string()),
                };
                if worker_find_epoch.load(Ordering::Relaxed) == request.ticket
                    && find_tx.send(Reply::Found(request.ticket, result)).is_err()
                {
                    break;
                }
            }
        });
        Self {
            io,
            search,
            outline,
            find,
            replies,
            epoch,
            outline_epoch,
            find_epoch,
        }
    }
    pub fn cancel_search(&self) -> u64 {
        self.epoch.fetch_add(1, Ordering::Relaxed) + 1
    }
    pub fn cancel_outline(&self) -> u64 {
        self.outline_epoch.fetch_add(1, Ordering::Relaxed) + 1
    }
    pub fn cancel_find(&self) -> u64 {
        self.find_epoch.fetch_add(1, Ordering::Relaxed) + 1
    }
}
impl Drop for Workers {
    fn drop(&mut self) {
        self.cancel_search();
        self.cancel_outline();
        self.cancel_find();
    }
}
