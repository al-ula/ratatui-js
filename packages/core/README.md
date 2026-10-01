# Application API

`createFrame(root)` validates existing protocol nodes. `row`, `column`, `block`,
`paragraph`, and `list` build those nodes with typed options. Validation happens
when constructing a frame; builders do not mutate inputs.

`startApplication(driver, {initialModel, view, update, rendered?})` takes
ownership of the driver and returns `{done, update, exit}`. `runApplication`
returns only `done` for applications driven entirely by events. Both draw
initially, serialize async model updates, invalidate on resize, coalesce queued
updates, and close in `finally`. Return `EXIT` from an update or call `exit()`
for explicit shutdown. `done` resolves with the final JavaScript model after
cleanup. Await it to observe callback, driver, and restoration failures. When an
application failure and cleanup failure coexist, both appear in an
`AggregateError`.

External `update(model => nextModel)` resolves after applying the change, before
its redraw completes. Changes queued during rendering share the next redraw.
Updates queued at exit reject. A view or update must finish its own async work;
exit cannot cancel arbitrary application callbacks.

The optional `rendered(model, result)` receives dimensions and native list
state. Return a model with updated list offsets/selections. This callback does
not itself invalidate the frame, avoiding an endless redraw cycle. New events
and explicit updates redraw using the updated state. Do not invoke and await
`app.update()` from within a runner callback: it would wait for itself.

See [`examples/application.ts`](../../examples/application.ts) for list
selection, resize redraw, keyboard exit, and Deno interrupt cleanup.
