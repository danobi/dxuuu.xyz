% Rewriting clipd

`clipd` is a distributed clipboard I wrote back in 2017. Distributed in this
context means that a server somewhere runs the clipd server and accepts
requests from clients to either:

* Store new data into the clipboard, or
* Grab the current data in the clipboard

This solved an annoying problem I had back then where I had too many computers
(still do, actually) and had difficulty sharing links between them. This was
not a very complicated piece of software (I think it took maybe two weekends).
Nonetheless, it still bitrot over the years and recently stopped working for
reasons I was too lazy to debug. I did, however, take it as a chance to rewrite
the code from Python to Rust, along with a bunch of other improvements along
the way.

It took about 6-8 hours to do the full rewrite and after having done so, I
thought it would be interesting to document the different design choices I've
made. Put differently, why current-me is better at programming than old-me four
years ago.

## Language and framework

We'll ignore the client in this section because the client is very simple in both
implementations: open a connection, write some bytes, read some bytes, print. The
server is more interesting to discuss.

#### Old server

The old server was written in Python and used
[`socketserver.TCPServer`](https://docs.python.org/3/library/socketserver.html)
with `socketserver.ThreadingMixIn`, meaning each connection was handled in
a separate thread. 

#### New server

For the new server, I used async Rust with [tokio](https://tokio.rs/) and
[`tokio::TcpListener`](https://docs.rs/tokio/1.13.0/tokio/net/struct.TcpListener.html)
on a single thread to drive the event loop. 

#### Commentary

Python as a language was an OK choice. Using `socketserver` is fine as
well, thought I might have chosen `asyncio` if I had to do things over in
Python. I'm not sure what the state of asyncio was back then.

I quite like Rust these days. Rust is pretty easy to write and reason about,
plus the memory and type safety is extremely appealing for a project I have no
intention of ever writing tests for. The extra efficiency from being a compiled
language is nice as well.  Using an async framework seems like a good choice
since clipd is entire I/O bound.

## Protocol

`clipd` was and still is TCP-based protocol. TCP remains an excellent choice
because I really don't want to deal with an unreliable connection. On top of
TCP, we have our own framed protocol. Framing is obviously necessary because
TCP is a streaming protocol, not a message based one. And delimiters seems
troublesome given we support arbitrary payloads.

#### Old protocol

```
`clipd` communicates over TCP using ASCII encoded data (please don't hate me,
utf-8 people). The wire format is as follows:

+---+------+-------+
|LEN|HEADER|PAYLOAD|
+---+------+-------+
```

where:

* `LEN` is the total number of characters in the message (`LEN` excluded).
* `HEADER` can be either `"PUSH"` or `"PULL"` for requests and `"OK"` or
  `"ERR"` for responses
* `PAYLOAD` is either empty, contains the clipboard payload, or the error
  message

#### New protocol

```
clipd protocol (request and response):

0       8      16
+-------+-------------------------+
| magic | type | optional payload |
+-------+-------------------------+

The payload, if present:

0     64
+-----+---------+
| len | payload |
+-----+---------+
```

where:

* `magic` is a clipd-specific magic value (integer)
* `type` is the type of request (integer)
* `len` is length of payload
* `payload` is opaque series of bytes


#### Commentary

Thinking about the old protocol now, I have a couple immediate thoughts:

* Using ASCII is plain bad because UTF-8 is clearly superior
* `LEN` is a strange field because any wire protocol usually has a fixed set of
  fields in the front and length is only needed if there is a variable sized
  field. Although not wrong, the placement is strange
* The text-based `HEADER` field is extremely strange, as it's extra bytes and
  could be replaced by an integer

One thing I learned from staring at the output from the old clipd server is
that there are a lot of scanners on the internet that will try probing (sending
bytes to) any open port on your server. The magic in the new protocol is there
to quickly drop invalid requests. The type in the new protocol is now an
integer to be more efficient. Similarly, the length field is now only included
when necessary.


## Protocol parsing

Protocol parsing is, well, parsing the bytes that come over the wire into
the protocol we have designed in the previous section. Although this needs
to happen in both the server and the client, the process is similar enough
in both binaries to describe together.

#### Old process

The raw bytes are both constructed and processed inline with the application
level logic. Meaning something like the following:

```python
req = str(len(HDR_PULL)) + HDR_PULL
req = bytes(req, 'ascii')
resp = _sock_send_recv(...)

hdr, payload = _parse_resp(resp)
if (hdr != HDR_OK):
  raise ClipdException(payload)
return payload
```

#### New process

The new code is structured to have a protocol "library" that hides the details
of how the requests and responses are sent over the wire. This "library" is
used in both the client and server. The library more or less looks like:

```rust
pub enum RequestFrame {
    /// A request to push bytes to the server
    Push(Vec<u8>),
    /// A request to pull the currently stored bytes on the server
    Pull,
}

impl RequestFrame {
    pub async fn from_socket(socket: &mut TcpStream) -> Result<RequestFrame>;
    pub fn to_bytes(&self) -> Vec<u8>;
}
```

`ResponseFrame` has an symmetric API except with different enum variants.

#### Commentary

The new way is obviously superior looking back after the fact. Mixing protocol
level details with the application logic leads to more complexity (not
necessarily in this simple application, but the principle still applies) as
well as duplicated logic in both client and server. It's a clear win to
separate concerns and share code between the client and server.

## Operation in production

"Operation" and "production" in this context means "how it's run" and "on my
server", respectively.

#### Old way

I ran the clipd server binary inside a session of tmux.

#### New way

I run clipd as a systemd system service.

#### Commentary

The old way was simpler (not by much) but easier to mess up. For example
if the server was restarted I would have to remember to run clipd again.

The new way is clearly better b/c systemd is designed to manage system daemons
(among other things). I suspect I didn't know systemd well enough back then
to know to make a systemd service.

## Final commentary

After the rewrite clipd feels much snappier and seems to work well again. I
found the process of exploring my old code interesting enough that I spent even
more time to write a post. So in a way I suppose I'm doubly glad I finally got
around to it.
