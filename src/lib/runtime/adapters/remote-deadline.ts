export class RemoteDeadlineExceeded extends Error {}

/** Race even noncompliant fetchers; an aborted HTTP request may still commit on the host. */
export async function withRemoteDeadline<T>(milliseconds: number, work: (signal: AbortSignal) => Promise<T>): Promise<T> {
  const controller = new AbortController();
  let timer: ReturnType<typeof setTimeout> | undefined;
  const deadline = new Promise<never>((_resolve, reject) => {
    timer = setTimeout(() => {
      reject(new RemoteDeadlineExceeded());
      controller.abort();
    }, milliseconds);
  });
  try {
    return await Promise.race([work(controller.signal), deadline]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}
