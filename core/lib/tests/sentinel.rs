use rocket::{*, error::ErrorKind::SentinelAborts};

#[get("/two")]
fn two_states(_one: State<u32>, _two: State<String>) {}

#[get("/one")]
fn one_state(_three: State<u8>) {}

#[async_test]
async fn state_sentinel_works() {
    let err = rocket::build()
        .configure(Config::debug_default())
        .mount("/", routes![two_states])
        .ignite().await
        .unwrap_err();

    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 2));

    let err = rocket::build()
        .configure(Config::debug_default())
        .mount("/", routes![two_states])
        .manage(String::new())
        .ignite().await
        .unwrap_err();

    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 1));

    let err = rocket::build()
        .configure(Config::debug_default())
        .mount("/", routes![two_states])
        .manage(1 as u32)
        .ignite().await
        .unwrap_err();

    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 1));

    let result = rocket::build()
        .configure(Config::debug_default())
        .mount("/", routes![two_states])
        .manage(String::new())
        .manage(1 as u32)
        .ignite().await;

    assert!(result.is_ok());

    let err = rocket::build()
        .configure(Config::debug_default())
        .mount("/", routes![one_state])
        .ignite().await
        .unwrap_err();

    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 1));

    let result = rocket::build()
        .configure(Config::debug_default())
        .mount("/", routes![one_state])
        .manage(1 as u8)
        .ignite().await;

    assert!(result.is_ok());

    let err = rocket::build()
        .configure(Config::debug_default())
        .mount("/", routes![one_state, two_states])
        .ignite().await
        .unwrap_err();

    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 3));

    let err = rocket::build()
        .configure(Config::debug_default())
        .mount("/", routes![one_state, two_states])
        .manage(1 as u32)
        .ignite().await
        .unwrap_err();

    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 2));

    let err = rocket::build()
        .configure(Config::debug_default())
        .mount("/", routes![one_state, two_states])
        .manage(1 as u8)
        .ignite().await
        .unwrap_err();

    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 2));

    let err = rocket::build()
        .configure(Config::debug_default())
        .mount("/", routes![one_state, two_states])
        .manage(1 as u32)
        .manage(1 as u8)
        .ignite().await
        .unwrap_err();

    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 1));

    let result = rocket::build()
        .configure(Config::debug_default())
        .mount("/", routes![one_state, two_states])
        .manage(1 as u32)
        .manage(1 as u8)
        .manage(String::new())
        .ignite().await;

    assert!(result.is_ok());
}

#[test]
fn inner_sentinels_detected() {
    use rocket::local::blocking::Client;

    #[derive(Responder)]
    struct MyThing<T>(T);

    struct ResponderSentinel;

    impl<'r, 'o: 'r> response::Responder<'r, 'o> for ResponderSentinel {
        fn respond_to(self, _: &'r Request<'_>) -> response::Result<'o> {
            todo!()
        }
    }

    impl Sentinel for ResponderSentinel {
        fn abort(_: &Rocket<Ignite>) -> bool {
            true
        }
    }

    #[get("/")]
    fn route() -> MyThing<ResponderSentinel> { todo!() }

    let err = Client::debug_with(routes![route]).unwrap_err();
    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 1));

    #[derive(Responder)]
    struct Inner<T>(T);

    #[get("/")]
    fn inner() -> MyThing<Inner<ResponderSentinel>> { todo!() }

    let err = Client::debug_with(routes![inner]).unwrap_err();
    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 1));

    #[get("/")]
    fn inner_either() -> Either<Inner<ResponderSentinel>, ResponderSentinel> { todo!() }

    let err = Client::debug_with(routes![inner_either]).unwrap_err();
    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 2));

    #[derive(Responder)]
    struct Block<T>(T);

    impl<T> Sentinel for Block<T> {
        fn abort(_: &Rocket<Ignite>) -> bool {
            false
        }
    }

    #[get("/")]
    fn blocked() -> Block<ResponderSentinel> { todo!() }

    Client::debug_with(routes![blocked]).expect("no sentinel errors");

    #[get("/a")]
    fn inner_b() -> Either<Inner<Block<ResponderSentinel>>, Block<ResponderSentinel>> {
        todo!()
    }

    #[get("/b")]
    fn inner_b2() -> Either<Block<Inner<ResponderSentinel>>, Block<ResponderSentinel>> {
        todo!()
    }

    Client::debug_with(routes![inner_b, inner_b2]).expect("no sentinel errors");

    #[get("/")]
    fn half_b() -> Either<Inner<ResponderSentinel>, Block<ResponderSentinel>> {
        todo!()
    }

    let err = Client::debug_with(routes![half_b]).unwrap_err();
    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 1));

    use rocket::response::Responder;

    #[get("/")]
    fn half_c<'r>() -> Either<
        Inner<impl Responder<'r, 'static>>,
        Result<ResponderSentinel, Inner<ResponderSentinel>>
    > {
        Either::Left(Inner(()))
    }

    let err = Client::debug_with(routes![half_c]).unwrap_err();
    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 2));

    #[get("/")]
    fn half_d<'r>() -> Either<
        Inner<impl Responder<'r, 'static>>,
        Result<Block<ResponderSentinel>, Inner<ResponderSentinel>>
    > {
        Either::Left(Inner(()))
    }

    let err = Client::debug_with(routes![half_d]).unwrap_err();
    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 1));

    // The special `Result` implementation.
    type MyResult = Result<ResponderSentinel, ResponderSentinel>;

    #[get("/")]
    fn half_e<'r>() -> Either<Inner<impl Responder<'r, 'static>>, MyResult> {
        Either::Left(Inner(()))
    }

    let err = Client::debug_with(routes![half_e]).unwrap_err();
    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 1));

    // Another specialized sentinel.

    #[get("/")] fn either_route() -> Either<ResponderSentinel, ResponderSentinel> { todo!() }
    let err = Client::debug_with(routes![either_route]).unwrap_err();
    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 1));

    #[get("/")] fn either_route2() -> Either<ResponderSentinel, ()> { todo!() }
    let err = Client::debug_with(routes![either_route2]).unwrap_err();
    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 1));

    #[get("/")] fn either_route3() -> Either<(), ResponderSentinel> { todo!() }
    let err = Client::debug_with(routes![either_route3]).unwrap_err();
    assert!(matches!(err.kind(), SentinelAborts(vec) if vec.len() == 1));

    #[get("/")] fn either_route4() -> Either<(), ()> { todo!() }
    Client::debug_with(routes![either_route4]).expect("no sentinel error");
}