package de.sve.backend.mail;

import com.google.auto.value.AutoValue;

import jakarta.activation.DataSource;
import jakarta.mail.util.ByteArrayDataSource;

/**
 * Creates an attachment for an email.
 *
 * @author mseele
 */
@AutoValue
public abstract class Attachment {

	public static Attachment create(String name, byte[] data, String mimeType) {
		return create(name, new ByteArrayDataSource(data, mimeType));
	}

	public static Attachment create(String name, DataSource dataSource) {
		return new AutoValue_Attachment(name, dataSource);
	}

	public abstract String name();

	public abstract DataSource dataSource();

}
