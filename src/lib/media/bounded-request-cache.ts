/** Bounded derived-media cache. Pending work is coalesced and never evicted while running.
 * If every slot is pending, replaceable thumbnail work stays a solid fill until remount.
 */
export class BoundedRequestCache<Value> {
  private readonly entries = new Map<string, { promise: Promise<Value | null>; pending: boolean; bytes: number }>();
  private bytes = 0;

  constructor(private readonly maxEntries: number, private readonly maxBytes: number, private readonly estimateBytes: (value: Value) => number) {}

  get size(): number { return this.entries.size; }
  get retainedBytes(): number { return this.bytes; }

  get(key: string, load: () => Promise<Value | null>): Promise<Value | null> {
    const cached = this.entries.get(key);
    if (cached) {
      this.entries.delete(key);
      this.entries.set(key, cached);
      return cached.promise;
    }
    while (this.entries.size >= this.maxEntries) {
      if (!this.evictSettled()) return Promise.resolve(null);
    }
    const entry = { promise: Promise.resolve(null) as Promise<Value | null>, pending: true, bytes: 0 };
    this.entries.set(key, entry);
    entry.promise = Promise.resolve().then(load).then((value) => {
      entry.pending = false;
      entry.bytes = value === null ? 0 : Math.max(0, this.estimateBytes(value));
      this.bytes += entry.bytes;
      while (this.bytes > this.maxBytes && this.evictSettled()) { /* discard oldest settled reports */ }
      return value;
    }, () => {
      this.entries.delete(key);
      return null;
    });
    return entry.promise;
  }

  private evictSettled(): boolean {
    for (const [key, entry] of this.entries) {
      if (entry.pending) continue;
      this.entries.delete(key);
      this.bytes -= entry.bytes;
      return true;
    }
    return false;
  }
}
