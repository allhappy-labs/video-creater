export function createTimelineFrameCoalescer<TInput, TResult>(input: {
  evaluate: (value: TInput) => TResult;
  deliver: (result: TResult) => void;
  requestFrame: (callback: FrameRequestCallback) => number;
  cancelFrame: (handle: number) => void;
}) {
  let pending: TInput | null = null;
  let handle: number | null = null;
  const run = () => {
    handle = null;
    if (pending === null) return null;
    const value = pending;
    pending = null;
    const result = input.evaluate(value);
    input.deliver(result);
    return result;
  };
  return {
    schedule(value: TInput) {
      pending = value;
      if (handle === null) handle = input.requestFrame(run);
    },
    flush(value?: TInput) {
      if (value !== undefined) pending = value;
      if (handle !== null) input.cancelFrame(handle);
      handle = null;
      return run();
    },
    cancel() {
      if (handle !== null) input.cancelFrame(handle);
      handle = null;
      pending = null;
    },
  };
}
