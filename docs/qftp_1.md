# Introduction 
This document describes version 1 of the qftp protocol to transfer, list and modify files on a remote server using the [QUIC](https://www.rfc-editor.org/rfc/rfc9000.html) transport protocol.

The purpose of this protocol is to utilize the cheap creation and teardown of [streams](https://www.rfc-editor.org/rfc/rfc9000.html#name-streams) to speed up the transfer of files, especially smaller files as well as utilizing QUICs by-default usage of TLS1.3 to ensure safe and encrypted transport.

**This protocol is a work in progress. Until this message is removed it shall be seen as unstable and rapidly changing**