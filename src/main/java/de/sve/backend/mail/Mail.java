package de.sve.backend.mail;

import javax.annotation.Nullable;

import com.google.auto.value.AutoValue;
import com.google.common.collect.ImmutableSet;

/**
 * Create and send an email.
 * 
 * @author mseele
 */
@AutoValue
public abstract class Mail {

	public static Builder via(MailAccount account) {
		return new AutoValue_Mail.Builder().sender(account);
	}

	public abstract MailAccount sender();

	public abstract ImmutableSet<String> to();

	public abstract ImmutableSet<String> bcc();

	@Nullable
	public abstract String replyTo();

	public abstract String subject();

	public abstract String content();

	@AutoValue.Builder
	public abstract static class Builder {

		abstract Builder sender(MailAccount value);

		public abstract Builder to(String... value);

		public abstract Builder bcc(String... value);

		public abstract Builder replyTo(String value);

		public abstract Builder subject(String value);

		public abstract Builder content(String value);

		abstract Mail build();

		public boolean send() {
			return Postman.deliver(build());
		}

	}

}
