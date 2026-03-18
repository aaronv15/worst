cargo build
name="worst"
profile="debug"
bin_path="./target/$profile/$name"
result=$("$bin_path" $*)
if [ -n "$result" ] && [ -z "${result##EXEC::*}" ]; then
   eval "${result#EXEC::}"
else
   echo "$result"
fi
