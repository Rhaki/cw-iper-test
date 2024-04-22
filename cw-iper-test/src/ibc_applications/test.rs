use cw_iper_test_macros::{ urls_int, Stargate};

#[derive(Stargate)]
#[stargate(name = "test", query_urls = TestQueryUrls, msgs_urls = TestMsgUrls)]
pub struct Test {}

#[urls_int]
pub enum TestMsgUrls {}

#[urls_int]
pub enum TestQueryUrls {}
