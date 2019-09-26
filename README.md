# What is this?

This is a docker registry, i.e. you can pull images from this registry
with e.g. `docker run` or in kubernetes.

This registry builds images on-the-fly based on a nix expression
found in a public or private repository.

I first explored this idea [here](https://github.com/teh/simci/blob/master/api-v2.py)
and there is also [nixery](http://nixery.dev/), a slightly different take on the same idea.
