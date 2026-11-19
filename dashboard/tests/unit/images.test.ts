import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// An image from the server (lib/api/images.ts): its URL in the browser; in the app, through the IPC transport, as a
// `data:` URL made once per URL. The transport is stubbed: where the page runs, and ipcFetch.
const where = vi.hoisted(() => ({ app: false }));
const { ipcFetch } = vi.hoisted(() => ({ ipcFetch: vi.fn<(url: string) => Promise<Response>>() }));
vi.mock('../../src/lib/api/http', () => ({
  get inApp() {
    return where.app;
  },
}));
vi.mock('../../src/lib/api/ipc', () => ({ ipcFetch }));

/** A fresh module each test: the URLs it made are its own */
async function images() {
  vi.resetModules();
  return import('../../src/lib/api/images');
}

const png = (bytes: number[]) => new Response(new Uint8Array(bytes), { headers: { 'Content-Type': 'image/png' } });

beforeEach(() => {
  where.app = false;
  ipcFetch.mockReset();
});

afterEach(() => vi.restoreAllMocks());

describe('imageSource', () => {
  it('in the browser is the URL itself, asking nothing', async () => {
    const { imageSource } = await images();
    expect(await imageSource('/p/p1/logo')).toBe('/p/p1/logo');
    expect(ipcFetch).not.toHaveBeenCalled();
  });

  it('in the app is the image as a data: URL', async () => {
    where.app = true;
    ipcFetch.mockResolvedValue(png([137, 80, 78, 71]));
    const { imageSource } = await images();
    expect(await imageSource('/p/p1/logo')).toBe('data:image/png;base64,iVBORw==');
    expect(ipcFetch).toHaveBeenCalledWith('/p/p1/logo');
  });

  it('in the app is asked once per URL, each URL its own', async () => {
    where.app = true;
    ipcFetch.mockImplementation(async (url) => png(url.includes('p1') ? [1] : [2]));
    const { imageSource } = await images();
    const [a, again, b] = await Promise.all([
      imageSource('/p/p1/logo'),
      imageSource('/p/p1/logo'),
      imageSource('/p/p2/logo'),
    ]);
    expect(a).toBe(again);
    expect(a).not.toBe(b);
    expect(ipcFetch).toHaveBeenCalledTimes(2);
  });

  it.each([
    ['answers an error', () => Promise.resolve(new Response('{}', { status: 404 }))],
    ['fails', () => Promise.reject(new Error('no app'))],
  ])('in the app is null when the app %s, and isn’t asked again', async (_, answer) => {
    where.app = true;
    ipcFetch.mockImplementation(answer);
    const { imageSource } = await images();
    expect(await imageSource('/p/p1/logo')).toBeNull();
    expect(await imageSource('/p/p1/logo')).toBeNull();
    expect(ipcFetch).toHaveBeenCalledTimes(1);
  });

  it('in the app is null when the image can’t be read', async () => {
    where.app = true;
    ipcFetch.mockResolvedValue(png([1]));
    vi.spyOn(FileReader.prototype, 'readAsDataURL').mockImplementation(function (this: FileReader) {
      this.onerror?.(new ProgressEvent('error') as ProgressEvent<FileReader>);
    });
    const { imageSource } = await images();
    expect(await imageSource('/p/p1/logo')).toBeNull();
  });
});
