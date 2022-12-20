# Introduction 
This document describes version 1 of the qftp protocol to transfer, list and modify files on a remote server using the [QUIC](https://www.rfc-editor.org/rfc/rfc9000.html) transport protocol.

The purpose of this protocol is to utilize the cheap creation and teardown of [streams](https://www.rfc-editor.org/rfc/rfc9000.html#name-streams) to speed up the transfer of files, especially smaller files as well as utilizing QUICs by-default usage of TLS1.3 to ensure safe and encrypted transport.

# Connection initiation and version negotiation
The client starts off by connecting to the server. QUIC is used for all server-client communication. Once the QUIC connection has been established the client opens a [bidirectional stream](https://www.rfc-editor.org/rfc/rfc9000.html#name-bidirectional-stream-states). This stream is called the `control message stream`. It is used over the entire duration of the qftp session.

The client then sends a [Hello Message](#hello-message) to the server.
The server responds with a [Version Message](#version-message) if a version is found that is both supported by client and server. Otherwise the server responds with an [Error Message](#error-message).
**TODO: Error Message explanation**  

# Messages
Messages are used to initiate requests. They are usually send from Client to Server. Most messages are send over the `control message stream`.

All messages start with a 1 byte unsigned integer corresponding to the message ID. The rest of the message depends on the message ID. See in the relevant section.

|Message ID|Message Name|Definition|
|----------|----------|------------|
|0x00      |HELLO     | [Hello Message](#hello-message)
|0x01      |VERSION   | [Version Message](#version-message)
|0x02      |ERROR     | [Error Message](#error-message)

## Hello Message
```
Hello Message {
    Message ID (1)
    Versions Length (1)
    Supported Versions (..)
}
```
The Hello Message has the message ID `0x00`. `Supported Versions` is an array of unsigned 8bit integers. The length of the array is specified by `Versions Length`.
## Version Message

## Error Message
**This protocol is a work in progress. Until this message is removed it shall be seen as unstable and rapidly changing**