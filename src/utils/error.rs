use std::fmt;


#[derive(Debug)]
pub enum MyError {
    Reqwest(reqwest::Error),
    IsahcReqwest(isahc::Error),
    ErrorStr(String),

}
impl From<reqwest::Error> for MyError {
    fn from(err: reqwest::Error) -> Self {
        MyError::Reqwest(err)
    }
}
impl From<isahc::Error> for MyError {
    fn from(err: isahc::Error) -> Self {
        MyError::IsahcReqwest(err)
    }
}
impl std::error::Error for MyError {}

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MyError::Reqwest(err) => write!(f, "Reqwest error: {}", err),
            MyError::IsahcReqwest(err) => write!(f, "Reqwest error: {}", err),
            MyError::ErrorStr(s) => write!(f, "{}", s),
        }
    }
}
