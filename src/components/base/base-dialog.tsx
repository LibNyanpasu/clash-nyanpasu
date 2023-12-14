import { LoadingButton } from "@mui/lab";
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  type SxProps,
  type Theme,
} from "@mui/material";
import { ReactNode } from "react";

interface Props {
  title: ReactNode;
  open: boolean;
  okBtn?: ReactNode;
  okBtnDisabled?: boolean;
  cancelBtn?: ReactNode;
  disableOk?: boolean;
  disableCancel?: boolean;
  disableFooter?: boolean;
  contentSx?: SxProps<Theme>;
  children?: ReactNode;
  loading?: boolean;
  onOk?: () => void;
  onCancel?: () => void;
  onClose?: () => void;
}

export interface DialogRef {
  open: () => void;
  close: () => void;
}

export function BaseDialog(props: Props) {
  const {
    open,
    title,
    children,
    okBtn,
    okBtnDisabled,
    cancelBtn,
    contentSx,
    disableCancel,
    disableOk,
    disableFooter,
    loading,
  } = props;

  return (
    <Dialog open={open} onClose={props.onClose}>
      <DialogTitle>{title}</DialogTitle>

      <DialogContent sx={contentSx}>{children}</DialogContent>

      {!disableFooter && (
        <DialogActions>
          {!disableCancel && (
            <Button variant="outlined" onClick={props.onCancel}>
              {cancelBtn}
            </Button>
          )}
          {!disableOk && (
            <LoadingButton
              disabled={loading || okBtnDisabled}
              loading={loading}
              variant="contained"
              onClick={props.onOk}
            >
              {okBtn}
            </LoadingButton>
          )}
        </DialogActions>
      )}
    </Dialog>
  );
}
