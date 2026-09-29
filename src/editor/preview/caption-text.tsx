interface CaptionWordStyle {
  readonly wordIndex: number;
  readonly scale: number;
  readonly opacity: number;
  readonly color: string;
}

/**
 * Caption text with emphasized words highlighted. Words are counted across whitespace-separated
 * tokens; a per-word style (active word animation) overrides the highlight colour, opacity and scale.
 */
export function CaptionText({
  text,
  emphasizedWordIndices = [],
  wordStyles = [],
}: {
  readonly text: string;
  readonly emphasizedWordIndices?: readonly number[] | undefined;
  readonly wordStyles?: readonly CaptionWordStyle[] | undefined;
}) {
  const emphasized = new Set(emphasizedWordIndices);
  const styles = new Map(wordStyles.map((style) => [style.wordIndex, style]));
  let wordIndex = 0;
  return (
    <>
      {text.split(/(\s+)/).map((token, tokenIndex) => {
        if (!token || /^\s+$/.test(token)) return token;
        const index = wordIndex;
        wordIndex += 1;
        const style = styles.get(index);
        if (!emphasized.has(index) && !style) return token;
        return (
          <span
            key={`${tokenIndex}-${token}`}
            data-emphasized-word={index}
            className="inline-block text-keyframe"
            style={style ? { color: style.color, opacity: style.opacity, transform: `scale(${style.scale})` } : undefined}
          >
            {token}
          </span>
        );
      })}
    </>
  );
}
