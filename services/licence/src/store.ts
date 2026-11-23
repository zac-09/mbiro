/**
 * What the service remembers. One interface, so the hosted database is one
 * adapter; `MemoryStore` serves tests and local runs and forgets on exit.
 *
 * Every method is one atomic step. A hosted adapter must keep that true
 * (a transaction per method), or two Macs activating at once could both take
 * the last place.
 */

/** A purchase, named by the payment provider and its order id. */
export interface Order {
  key: string;
  licence: string;
  email: string;
  /** Whether the licence email went out. A repeated webhook sends it if not. */
  emailed: boolean;
}

export type Activation = "ok" | "device_limit" | "revoked";

export interface Store {
  /** Gives `device` a place on the licence if it has one or one is free. */
  activate(licence: string, device: string, limit: number, at: Date): Promise<Activation>;
  /** Frees the device's place. Unknown licences and devices are not an error. */
  deactivate(licence: string, device: string): Promise<void>;
  /** Ids of every refunded licence. */
  revoked(): Promise<string[]>;
  /** Records an order unless it is already there; returns the one that is kept. */
  keepOrder(ref: string, order: Order): Promise<Order>;
  markEmailed(ref: string): Promise<void>;
  /** Marks an order's licence refunded. False if the order is unknown. */
  revokeOrder(ref: string): Promise<boolean>;
}

export class MemoryStore implements Store {
  private readonly devices = new Map<string, Map<string, Date>>();
  private readonly orders = new Map<string, Order>();
  private readonly refunded = new Set<string>();

  activate(licence: string, device: string, limit: number, at: Date): Promise<Activation> {
    if (this.refunded.has(licence)) return Promise.resolve("revoked");
    const places = this.devices.get(licence) ?? new Map<string, Date>();
    if (!places.has(device)) {
      if (places.size >= limit) return Promise.resolve("device_limit");
      places.set(device, at);
      this.devices.set(licence, places);
    }
    return Promise.resolve("ok");
  }

  deactivate(licence: string, device: string): Promise<void> {
    this.devices.get(licence)?.delete(device);
    return Promise.resolve();
  }

  revoked(): Promise<string[]> {
    return Promise.resolve([...this.refunded].sort());
  }

  keepOrder(ref: string, order: Order): Promise<Order> {
    const kept = this.orders.get(ref) ?? { ...order };
    this.orders.set(ref, kept);
    return Promise.resolve({ ...kept });
  }

  markEmailed(ref: string): Promise<void> {
    const order = this.orders.get(ref);
    if (order) order.emailed = true;
    return Promise.resolve();
  }

  revokeOrder(ref: string): Promise<boolean> {
    const order = this.orders.get(ref);
    if (!order) return Promise.resolve(false);
    this.refunded.add(order.licence);
    return Promise.resolve(true);
  }
}
