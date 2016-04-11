extern crate mio;
extern crate http_muncher;
extern crate rustc_serialize;
extern crate sha1;

use mio::*;
use mio::tcp::*;
use std::net::SocketAddr;
use std::collections::HashMap;
use http_muncher::{Parser, ParserHandler};
use rustc_serialize::base64::{ToBase64,STANDARD};
use std::cell::RefCell;
use std::rc::Rc;
use std::fmt;

const SERVER_TOCKEN:Token = Token(0);

#[derive(PartialEq)]
enum ClientState{
    AwaitingHadshake,
    HandshakeResponse,
    Connected
}
struct HttpParser{
    current_key: Option<String>,
    headers:Rc<RefCell<HashMap<String,String>>>
}

impl ParserHandler for HttpParser{
    fn on_header_field(&mut self, s: &[u8]) -> bool{
        self.current_key= Some(std::str::from_utf8(s).unwrap().to_string());
        true
    }

    fn on_header_value(&mut self, s: &[u8]) -> bool{
        self.headers.borrow_mut().insert(self.current_key.clone().unwrap(),
            std::str::from_utf8(s).unwrap().to_string());
        true
    }
    fn on_headers_complete(&mut self) -> bool{
        false
    }
}

struct WebSocketClient{
    socket: TcpStream,
    http_parser: Parser<HttpParser>,
    //adding the headers declaration to the WebSocketClient struct
    headers:Rc<RefCell<HashMap<String,String>>>,
    //adding the event property
    interest: EventSet,
    //a client state
    state:ClientState
}

impl WebSocketClient {
    fn new(socket:TcpStream) ->WebSocketClient {
        let headers = Rc::new(RefCell::new(HashMap::new()));

        WebSocketClient{
            socket: socket,
            //Making a first clone of the headers variable to read its contents
            headers: headers.clone(),
            //initial events that interste us
            interest: EventSet::readable(),
            //initial state
            state: ClientState::AwaitingHadshake,

            http_parser: Parser::request(HttpParser{
                current_key:None,
                //and the second clone to write new headers to it
                headers: headers.clone()
            })
        }
    }
    fn read(&mut self){
        loop{
            let mut buf = [0;2048];
            match self.socket.try_read(&mut buf) {
                Err(e) => {
                    println!("Error while reading socket: {:?}", e );
                    return
                },
                Ok(None) =>
                    //socket buffer  has got  no more bytes
                    break,
                Ok(Some(len))=>{
                    self.http_parser.parse(&buf[0..len]);
                    if self.http_parser.is_upgrade(){
                        //change the current state
                        self.state = ClientState::HandshakeResponse;
                        //change current interest to 'Writable'
                        self.interest.remove(EventSet::readable());
                        self.interest.insert(EventSet::writable());

                        break;
                    }
                }
            }
        }
    }

    fn write(&mut self){
        //get the headers HashMap from the Rc<RefCell<..>>wratter
        let headers = self.headers.borrow();

        //find the header that interests us, and generate the key fro its value
        let response_key = gen_key(&headers.get("Sec-WebSocket-Key").unwrap());

        //using the special function to format the string, able to find analogies in many other languates, but in Rust it is performed
        //at the complie time with the power of macros.
        let response = fmt::format(format_args!("HTTP/1.1 101 Switching Protocols\r\n\
                                                 Connection: Upgrade\r\n\
                                                 Sec-WebSocket-Accept: {}\r\n\
                                                 Upgrade: websocket\r\n\r\n", response_key));
        self.socket.try_write(response.as_bytes()).unwrap();
        //change the state
        self.state = ClientState::Connected;
        //Change the interset back to 'readable()':
        self.interest.remove(EventSet::writable());
        self.interest.insert(EventSet::readable());
    }
}

struct WebSocketServer{
    socket: TcpListener,
    clients: HashMap<Token,WebSocketClient>,
    token_counter: usize
}

impl Handler for WebSocketServer {
 // Traits can have useful default implementations, so in fact the handler
 // interface requires us to provide only two things: concrete types for
 // timeouts and messages.
 type Timeout = usize;
 type Message = ();

 fn ready(&mut self, event_loop: &mut EventLoop<WebSocketServer>,
                            token: Token, events: EventSet) {
    if events.is_readable(){
        match token {
            SERVER_TOCKEN => {
                let client_socket = match self.socket.accept(){
                    Err(e) => {
                        println!("Accept error: {}", e);
                        return;
                    },
                    Ok(None) => unreachable!("Accept has returned 'None'"),
                    Ok(Some((sock,addr)))=>sock
                };

                self.token_counter +=1;
                let new_token = Token(self.token_counter);

                self.clients.insert(new_token, WebSocketClient::new(client_socket));
                event_loop.register(&self.clients[&new_token].socket,
                                       new_token,EventSet::readable(),
                                       PollOpt::edge()|PollOpt::oneshot()).unwrap();

            },
            token => {
                let mut client = self.clients.get_mut(&token).unwrap();
                client.read();
                event_loop.reregister(&client.socket, token, client.interest,PollOpt::edge()|PollOpt::oneshot()).unwrap();
            }
        }
    }

    if events.is_writable(){
        let mut client= self.clients.get_mut(&token).unwrap();
        client.write();
        event_loop.reregister(&client.socket, token, client.interest,
                                PollOpt::edge()|PollOpt::oneshot()).unwrap();
    }

 }
}

fn gen_key(key:&String) -> String{
    let mut m = sha1::Sha1::new();
    let mut buf = [0u8; 20];

    m.update(key.as_bytes());
    m.update("258EAFA5-E914-47DA-95CA-C5AB0DC85B11".as_bytes());
    m.output(&mut buf);

    return buf.to_base64(STANDARD);
}

fn main() {
    let address = "0.0.0.0:10000".parse::<SocketAddr>().unwrap();
    let server_socket = TcpListener::bind(&address).unwrap();
    let mut event_loop = EventLoop::new().unwrap();
    // Create a new instance of our handler struct:
    let mut server = WebSocketServer{
        token_counter:1,//Starting the token count from 1
        clients: HashMap::new(), //creating an empty HashMap
        socket: server_socket //handling the ownership of the socket to the struct
    };
    // ... and provide the event loop with a mutable reference to it:
    event_loop.register(&server.socket,
                    SERVER_TOCKEN,
                    EventSet::readable(),
                    PollOpt::edge()).unwrap();

    event_loop.run(&mut server).unwrap();

}
