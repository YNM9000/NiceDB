declare module 'utif' {
  interface Ifd {
    width: number;
    height: number;
  }
  const UTIF: {
    decode(buffer: ArrayBuffer): Ifd[];
    decodeImage(buffer: ArrayBuffer, ifd: Ifd, ifds?: Ifd[]): void;
    toRGBA8(ifd: Ifd): Uint8Array;
  };
  export default UTIF;
}
