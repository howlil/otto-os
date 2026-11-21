type TProps = {
  id: string;
  en: string;
};

export function T({ id, en }: TProps) {
  return (
    <>
      <span className="l-id">{id}</span>
      <span className="l-en">{en}</span>
    </>
  );
}
