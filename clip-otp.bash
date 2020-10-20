# Password store extension to use clip-otp instead of xclip
#
# Drop into ~/.password-store/.extensions
# and invoke as `pass clip-otp -c ...`

function clip() {
	coproc (echo -n "$1" | clip-otp)
	read -u $COPROC MSG
	echo "$MSG"
	disown
}

cmd_show "$@"
