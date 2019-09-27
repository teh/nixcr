# What is this?

This is a docker registry, i.e. you can pull images from this registry
with e.g. `docker run` or in kubernetes.

This registry builds images on-the-fly based on a nix expression
found in a public or private repository.

I first explored this idea [here](https://github.com/teh/simci/blob/master/api-v2.py)
and there is also [nixery](http://nixery.dev/), a slightly different take on the same idea.

# Configuration

The service needs:

1. A directory to store layer blobs
2. A directory to cache `git clone` output
3. A set of repository descriptions

The invocation looks e.g. like this:

```sh
 nixcr \
     --blob-root /tmp/blob-cache/ \
     --repo-root /tmp/repo-cache/ \
     # public repo:
     --repo nixpkgs,https://github.com/NixOS/nixpkgs \
     #      ^^^^^^^ lookup key
     #              ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ repository for that key
     # private repo:
     --repo nixcr,git@github.com:private/private.git,/keys/id_rsa
     #      ^^^^^ lookup key
     #            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ repository for key
     #                                               ^^^^^^^^^^^^ path to private ssh deploy key
```

For the example above you could do e.g.

```sh
docker run -it 127.0.0.1:8888/nixpkgs/cafe1111:hello
        #      ^^^^^^^^^^^^^^ host + port where nixcr is running
        #                    ^^^^^^^^ lookup key (see above)
        #                             ^^^^^^^^ commit
        #                                      ^^^^^ nix attribute path
```
