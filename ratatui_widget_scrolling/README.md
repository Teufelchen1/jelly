# Ratatui Widget Scrolling

A ratatui widget made for rendering huge amounts of log lines, where each line is an arbitrary widget.

### Features

* Autoscrolls down as more log entries come in ("sticky behavior").
* Takes care of partially visible elements -> can scroll character line wise.
* The height of each rendered line can be dynamic.
* The render height may vary between each other and between renderings.
* Caches the last heights in relation to the frames width.
* Assuming perfect cache hits, only the visible elements `k` will be rendered: roughly `O(k/n)`.
* Assuming zero cache hits, all elements will be rendered at least once and at most log length times: roughly `O(n²)`.

Built for the specific use case of [`Jelly`](https://github.com/teufelchen1/jelly).
Should work for other projects too.

Feel free to open issues.

## Usage

### Automatic

The easiest way to use this is via `ScrollState`. It will track the scroll position, if you are currently in "sticky" mode and it will tell you if scrolling made a change (e.g. can't scroll past the last element). Ofcourse it also handles the rendering for you.

```Rust
// Our log, the datatype does not matter.
let log = vec![Foo::new("Four lines"), Foo::new("Five lines"), Foo::new("Three lines")];

let mut scroll_state = ScrollState::new();

// Scroll what ever you like
scroll_state.scroll_up();

// Ratatui rendering call
terminal.draw(|frame| my_draw_func(&mut scroll_state, &log, frame))
        .unwrap();

fn my_draw_func(scroll_state: &mut ScrollState, log: &[Foo], frame: &mut Frame) {
	// you see, the datatype of the log does not matter, as you have to provide a closure which
	// returns the needed height for rendering the element and an `impl Widget` that renders it.
	scroll_state.render(frame, frame.area(), log, |foo| {
		let height = foo.get_height();
		let widget = foo.get_widget();
		(height, widget)
	});
}
```

### Manual

You can do the rendering steps manually, if needed. 

#### Do everything manually: Which element goes where?

The `get_areas_to_render_from_scroll_position()` function takes the area where you want to render the log, how far you have scrolled and a list of the heights of each element in you log.
With that, it calculates which elements will be shown and where they should be rendered.

```Rust
// Our visible are 10x5
let area = Rect {
    x: 0,
    y: 0,
    height: 10,
    width: 5,
};
// We have scrolled to the bottom, that is the most recent log entries
let scroll_offset = 0;
// Element 0 (the oldest in the log) is 4 lines tall,
// Element 1 is 5 lines and the youngest at index 2 is 3 lines tall
let height_log = [4, 5, 3];

//  With this, we are all set up and expect this result:
// 
// |00000| the oldest element, index 0, is 4 lines tall
// |00000| only its lower two lines are visible
// +-----+ start of our visible area 10x5
// |00000|
// |00000|
// |11111| the second element, index 1, is 5 lines tall
// |11111| it is fully visible
// |11111|
// |11111|
// |11111|
// |22222| the youngest element, index 2, is 3 lines tall
// |22222| it is fully visible
// |22222|
// +-----+ end of our visible area


// `partial_draw_top` is the oldest log entry that needs to be rendered and it is only partially visible
// `partial_draw_bottom` is the equivalent for the youngest that is partially visible
// `full_draw_middle` contains all elements that are fully visible
let (partial_draw_top, full_draw_middle, partial_draw_bottom) =
    get_areas_to_render_from_scroll_position(area, scroll_offset, &height_log);

// As we expect, there is an element that is at the top and only partially visible
assert!(partial_draw_top.is_some());
let (index, area) = partial_draw_top.unwrap();
// Its the oldest element, index 0
assert_eq!(index, 0);
// We are supposed to draw it at coordinates (0,0), with a width of up to 5 and only 2 lines height 
assert_eq!(
    area,
    Rect {
        x: 0,
        y: 0,
        height: 2,
        width: 5,
    }
);

let (range, area) = full_draw_middle.unwrap();
// In the middle, we are told that element 1 and 2 are fully visible
assert_eq!(range, 1..3);
// We need to draw all of the at coordinates (0,2), width up to 5 and height of 8 lines
assert_eq!(
    area,
    Rect {
        x: 0,
        y: 2,
        height: 5 + 3,
        width: 5,
    }
);

// No elements left, which makes sense since we are scrolled to the bottom
assert!(partial_draw_bottom.is_none());
```

With that information you can now render your elements into the provided areas.

#### Manual rendering: I like tracking state myself

You can try to render a log by repeatedly calling `try_render_scroll_state()`. It will use `get_areas_to_render_from_scroll_position()` internally, so you don't need to worry about layouting.

```Rust
// get the frame you want to render into from ratatui (typically `terminal.draw(|frame|{..}))`)
let mut frame: Frame = ratatui::..(); 
// The area within that frame, where we want to render the log
let area = Rect { ... };
// Our scroll offset
let scroll_offset = ...;
// Our actual log, the datatype does not matter
let log = [Foo::new("Four lines"), Foo::new("Five lines"), Foo::new("Three lines")];
// ... and also the height log, but we need it mutable as it is used as a cache
let mut height_log = vec![log[0].get_height(), log[1].get_height(), log[2].get_height()];

// Since we are only trying to render, we might need to try multiple times
loop {
	let result = try_render_scroll_state(
		&mut frame,
		area,
		scroll_offset,
		&mut height_log,
		&log,
		// you see, the datatype of the log does not matter, as you have to provide a closure which
		// returns the needed height for rendering the element and an `impl Widget` that renders it.
		|foo| {
			let height = foo.get_height();
			let widget = foo.get_widget();
			(height, widget)
		});

	if result.is_ok() {
		// Nice, everything is rendered, we can exit the loop
		break;
	} else {
		// If the result is an error, we need to try rendering again.
		// An error occours when the heights provided in the `height_log` do not match the results
		// from the closure.  
		// Each time, the `height_log` gets updated with the freshly calculated heights.
		// Eventually all heights will be correct.
	}
}

```

The algorithm looks somewhat like this: 
```txt 		
                                     |
┌────────┐                  ┌────────▼────────┐
│ Height │ Get last heights │ Calculate which │
│ Cache  ├──────────────────► Elements will   │
└───▲────┘                  │ be visible      │
    │                       └────────┬────────┘
    │                                │
    │                       ┌────────▼────────┐
    │                       │ Try to render   │
    │                       │ the visible     │
┌───┴─────────┐             │ elements        │
│ Update the  │             └────────┬────────┘
│ cache with  │                      │
│ new heights │             ┌────────▼────────┐
└───▲─────────┘             │ Does the height │
    │          ┌──┐         │ of rendered     │
    └──────────┤No◄─────────┤ elements match  │
               └──┘         │ the height in   │
                            │ the cache?      │
                            └────────┬────────┘
                                     │
                                   ┌─▼─┐
                                   │Yes│
                                   └─┬─┘
                                     │
                              ┌──────▼──────┐
                              │ Display the │
                              │ result!     │
                              └─────────────┘
```

### Limitations

* The caching assumes a growing log and does not account for the deletion of elements. In such cases, the behavior is undefined.
* For a given area width, the required height for fully rendering an element must be deterministic.
