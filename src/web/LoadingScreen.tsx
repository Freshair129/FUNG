import { FungLogo } from "../components/FungLogo";
import "./LoadingScreen.css";

type LoadingScreenProps = {
  message?: string;
};

export function LoadingScreen({ message }: LoadingScreenProps) {
  return (
    <div className="loading-screen">
      <FungLogo size={48} />
      <div className="loading-spinner" />
      {message && <p className="loading-message">{message}</p>}
    </div>
  );
}
