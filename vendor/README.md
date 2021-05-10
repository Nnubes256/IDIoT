**Note**: if you are one of the maintainers behind `rust-libp2p`, please keep in mind all of the vendored dependencies below contain modifications that
    
1. are hacky, and
2. possibly go against the will of Saint Ferris or something.

Enter at your own risk!

(However, if you are looking for one, this vendored version of `libp2p-mdns` contains badly bolted-on support for custom mDNS service names, which I needed for good private net discovery support like in the NodeJS bindings. Perhaps they may work for you too, I dunno. Keep in mind this project uses a pre-stable version though)

(As for `request-response`... look, I really needed to directly access those channels, okay? xD)