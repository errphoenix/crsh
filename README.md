# CRSH 
### Centralised Remote SHell
A lightweight and fast remote shell protocol that doesn't expose any senders or receivers directly by routing commands through a central server. Written in Rust.

## Crates
* `crsh`: the receiving CRSH client agent that polls from the server and executes commands.
* `crsh-core`: core API containing shared types and logic used across all crates.
* `crsh-server`: the CRSH routing server.
* `crsh-tx`: optional CLI tool to interact with CRSH sessions (as a sender).
* `crsh-gui`: optional GUI client available for Linux & Android (made in Tauri) to interact with CRSH sessions as a sender.

## Overview
1. Senders (clients) submit commands to the routing server, either through `crsh-tx`, `crsh-gui`, or sending HTTP GET/POST requests (e.g. through curl).
2. Receivers (agents) poll the routing server at a fixed interval to receive and consume **their** command queue.
3. The agents will execute each command serially, one after the other, and upload each command's standard output (out and err) to the routing server. The standard outputs are uploaded for each command *after* it finishes executing.
4. Clients can query the history containing the standard outputs of all agents.

This design avoids NAT/port-forwarding, as all communication is performed through outbound HTTP requests.

### The Command Queue
When a client sends a command over to the server, it is added to the command queue. The command queue is a map, and the server will push the command to the respectful entry or entries (to one agent if a token is specified, to all agents if it's a broadcast).
Eventually, the agent(s) will poll their commands, consuming the contents of their queue and executing each command after the other.
If an agent is not connected, the commands will stay in the queue until the agent connects and consumes them as long as the agent is recognised (check the section below for more info on persistence).
If the server shuts down, the whole command queue is lost.

### Storage/Persistence
The client, server, and agent store little to no data on disk, but manage to maintain a certain degree of persistence.

- **Server storage**
  * **Server key**: stores the server key in plain text after it is generated. Subsequent server launches will load the stored key, but will generate (and write on file) a new one if the key is missing or malformed.
  * **Registered agents**: list of recognized agents tokens. Each time an agent authenticates to the server its token is added onto the list.
- **Agent storage**
  * **Token**: stores the agent's token when it authenticates to a server for the first time and will keep using that token in subsequent sessions. The token is global and will be used for all servers.

## How to use
### The Server
The server is the central core that connects the various clients and agents, it is the only part of the system that exposes itself by listening to inbound traffic while sending zero outbound requests.
It's a simple router with various rest-like endpoints, most of them requiring JSON data in the request body:
#### /hello 
Authenticates an agent and records it onto the server. Requires a name (can be anything), the access key, and, optionally, a uuid-v4 valid token (if absent a token will be generated). It returns the agent's token if successful; else the reason why it failed.
Example body:
```json
{
  "client": "Cool dude",
  "key": 67674
}
```
Produces:
```json
{
  "state": "Success",
  "token": "generated-uuid-v4-token"
}
```

#### /cmd
Sends a command onto the server's command queue, either a broadcast to all agents or to only one.
Example body:
```json
{
  "type": "Broadcast",
  "cmd": "echo 67"
}
```
Or to a single target:
```json
{
  "type": "Single",
  "token": "registered-agent-token",
  "cmd": "echo 67 but only here"
}
```

#### /poll
Consumes the list of commands for the given agent. It can returns either a 'Success' state, containing the command `queue`; or an 'EmptyQueue' state, that contains nothing; or a 'Failure' state that contains the `reason`.
Example body:
```json
{
  "token": "registered-agent-token"
}
```
Produces: 
```json
{
  "state": "Success",
  "queue": ["echo 67", "echo 67 but only here"]
}
```

### The Agent
The agent is the client that polls commands from the server, executes them, and uploads its output(s); all while holding little to no state. 
For an agent to connect to a server, it will be required to provide the server's access key.
An agent can be launched by using the `crsh` binary, currently available for Linux and Windows.
```bash
crsh --addr http://ADDRESS:PORT/ACCESS_KEY
```
You can also specify the interval the agent will be polling to in milliseconds with the `--interval` argument:
```bash
crsh --interval 1500 --addr http://ADDRESS:PORT/ACCESS_KEY
```
The default interval is 800ms.

### The Client
This is the sending client (referred to as tx-client for this reason, to not confuse it with the receiving client, the agent).
The client is available both as a CLI tool in `crsh-tx` for Linux and Windows and as a GUI in `crsh-gui` for Linux and Android.
The `crsh-tx` client does not perform any fixed polling or automatic requests, they are all sent out per user request through the `cmd`, `reset`, and `query` commands. 
Meanwhile, the `crsh-gui` client queries the history of the set remote at a fixed interval.
