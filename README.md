# Wingchain

[![Build Status](https://api.travis-ci.org/wingchain/wingchain.svg?branch=master)](https://travis-ci.org/wingchain/wingchain)
[![crates.io](https://img.shields.io/crates/v/wingchain?label=latest)](https://crates.io/crates/wingchain)
![Version](https://img.shields.io/badge/rustc-1.50.0--nightly-brightgreen)
![Apache 2.0 licensed](https://img.shields.io/crates/l/wingchain.svg)

Wingchain is an open source, easy to start and ready to scale block chain.

# Motivation

Wingchain is committed to becoming the first choice for `trustless` storage 
products, and obtaining the achievements MySQL and Redis have got in their 
respective area.

<table>
<tr>
    <td><b>Who</b></td>
    <td><b>How</b></td>
    <td><b>Why</b></td>
    <td><b>Where</b></td>
    <td><b>What</b></td>
</tr>
<tr>
    <td>Redis</td>
    <td>KV hight-speed storage</td>
    <td>Data access efficiency</td>
    <td>In memory</td>
    <td>Volatile data</td>
</tr>
<tr>
    <td>MySQL</td>
    <td>Structured persistent storage</td>
    <td>Data management efficiency</td>
    <td>In disk of single owner</td>
    <td>Mutable data</td>
</tr>
<tr>
    <td><b>Wingchain</b></td>
    <td><b>Trustless shared storage</b></td>
    <td><b>Data collaboration efficiency</b></td>
    <td><b>In disks of multiple owners</b></td>
    <td><b>Verified immutable data</b></td>
</tr>
</table>

# Positioning

Wingchain is not
 - framework (like Substrate, Tendermint)
 - platform (like Polkadot, Cosmos)

Wingchain is an out-of-the-box product.

# Features

## Easy to start
  
- Familiar paradigm
  
  Block, Address based identity, Account model, MPT based world state, VM, 
  Smart contract, Event, Bootnodes.
    
- Configurable and customizable cryptography

    - Digest algorithm: Blake2b / SM3 / Custom with dynamic link library 

    - Signature algorithm: Ed25519 / SM2 / Custom with dynamic link library

    - Address algorithm: Public key / Public key hash / Custom with dynamic 
      link library

- Configurable consensus

    - POA
      
      for trial cases.
    
    - Raft
      
      for crash fault tolerance cases. 
    
    - Hotstuff (WIP)
      
      for byzantine fault tolerance cases.
    
## Ready to scale

- Smart contract

    - Webassembly based 
    
    - Nestable
    
    - Upgradable
    
- Storage scale out

    - Multi partitions with different paths
    
    - Work together with distributed block storage
    
- High performance
    
    5000+ TPS

# License

Wingchain is under the Apache 2.0 license. See the [LICENSE](./LICENSE) 
file for details.

