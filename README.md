Running
-------

This application expects to find the shaders in the current working directory, so to run, do:

	cd src
	cargo run

TODO
----

- Calculate fractal on a different thread than UI to allow smoother interaction
- Calculate fractal in cached tiles so that panning and zooming keeps existing
  content and adds in new content as it becomes available
- Zoom around mouse pointer (keep the point under the mouse pointer the same)
- Extend point class with vector and scalar operations
