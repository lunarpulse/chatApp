extern crate mio;

use mio::*;
use mio::tcp::*;
use std::net::SocketAddr;
use std::collections::HashMap;

struct WebSocketServer{
    socket: TcpListener,
    clients: HashMap<Token,TcpStream>,
    token_counter: usize
}

const SERVER_TOCKEN:Token = Token(0);

impl Handler for WebSocketServer {

 // Traits can have useful default implementations, so in fact the handler
 // interface requires us to provide only two things: concrete types for
 // timeouts and messages.
 // We're not ready to cover these fancy details, and we wouldn't get to them
 // anytime soon, so let's get along with the defaults from the mio examples:
 type Timeout = usize;
 type Message = ();

 fn ready(&mut self, event_loop: &mut EventLoop<WebSocketServer>,
                            token: Token, events: EventSet) {
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

             self.clients.insert(new_token, client_socket);
             event_loop.register(&self.clients[&new_token],
                                    new_token,EventSet::readable(),
                                    PollOpt::edge()|PollOpt::oneshot()).unwrap();

         }
     }
 }
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
